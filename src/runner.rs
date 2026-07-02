use std::process::{Command, Stdio};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{self, File};
use chrono::Local;
use crate::types::{RunConfig, SearchMode};

mod config_img {
    pub use crate::types::RunConfig;

    const DEFAULT_IMG: &'static str = "seerbio/radiant-fulcrum:latest";

    impl RunConfig {
        pub fn get_img(&self) -> String {
            self.img.to_owned().unwrap_or(DEFAULT_IMG.to_string())
        }
    }
}

/// Maps host directories to container mount points and provides path remapping
struct VolumeMapper {
    /// Maps host directory -> container mount point
    mounts: HashMap<PathBuf, PathBuf>,
    mount_counter: usize,
}

impl VolumeMapper {
    fn new() -> Self {
        Self {
            mounts: HashMap::new(),
            mount_counter: 0,
        }
    }

    fn canonicalize_path(host_path: &str) -> io::Result<PathBuf> {
        let path = Path::new(host_path);
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        Ok(abs_path.canonicalize().unwrap_or(abs_path))
    }

    fn get_or_create_mount(&mut self, host_dir: PathBuf) -> PathBuf {
        if let Some(existing) = self.mounts.get(&host_dir) {
            existing.clone()
        } else {
            let mount_point = PathBuf::from(format!("/data{}", self.mount_counter));
            self.mount_counter += 1;
            self.mounts.insert(host_dir, mount_point.clone());
            mount_point
        }
    }

    /// Registers a file path and returns the remapped container path
    fn remap_file(&mut self, host_path: &str) -> io::Result<String> {
        let abs_path = Self::canonicalize_path(host_path)?;

        let parent = abs_path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Path has no parent directory")
        })?;

        let mount_point = self.get_or_create_mount(parent.to_path_buf());

        let file_name = abs_path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Path has no file name")
        })?;

        Ok(mount_point.join(file_name).to_string_lossy().to_string())
    }

    /// Registers a directory path and returns the remapped container path
    fn remap_dir(&mut self, host_path: &str) -> io::Result<String> {
        let abs_path = Self::canonicalize_path(host_path)?;
        let mount_point = self.get_or_create_mount(abs_path);
        Ok(mount_point.to_string_lossy().to_string())
    }

    /// Returns the docker volume arguments (-v host:container for each mount)
    fn volume_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (host_dir, container_dir) in &self.mounts {
            args.push("-v".to_string());
            args.push(format!("{}:{}", host_dir.display(), container_dir.display()));
        }
        args
    }
}

pub fn run_radiant_fulcrum<F>(config: RunConfig, mut on_output: F) -> io::Result<i32>
where
    F: FnMut(&str),
{
    let img = config.get_img();

    let dry_run = std::env::args().any(|arg| arg == "--dry-run");

    // Check that we can run docker
    match run_command(&mut |_| {}, Command::new("which").arg("docker"), &mut None) {
        Ok(0) =>  {}
        _ => {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Unable to locate `docker` executable"));
        }
    }

    if config.check_image_updates {
        if !dry_run {
            match pull_image(&img, &mut on_output) {
                Ok(0) => {}
                Ok(i) => {
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Unable to check for Docker image updates! Exit code {}", i)));
                }
                Err(e) => {
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Unable to check for Docker image updates! {}", e)));
                }
            }
        } else {
            on_output("[dry-run] Skipping image update check.")
        }
    }

    // Check that the image can be run, and get the Radiant version
    let mut version_check = Command::new("docker");
    version_check.args(vec![
            "run".to_string(),
            "--rm".to_string(),
            img.to_owned(),
            "bash".to_string(),
            "-c".to_string(),
            r"apt-cache policy radiantdia | grep -Po '(?<=Installed: )((?:\d+\.)*\d+)'".to_string(),
        ]);

    // Note: re-enable this once we're able to get a useful version number for Radiant.
    let mut ver: Option<String>  = None;
    match run_command(&mut |s| { ver = Some(s.to_string()) }, &mut version_check, &mut None) {
        Ok(0) =>  {
            let msg = format!("Found Radiant version {}", ver.map(|s| s.to_string()).unwrap_or("UNKNOWN".to_string()));
            on_output(msg.as_str());
        }
        Ok(i) => {
            return Err(io::Error::new(io::ErrorKind::Other, format!("Unable to run Radiant Docker image! Exit code {}", i)));
        }
        Err(e) => {
            return Err(io::Error::new(io::ErrorKind::Other, format!("Unable to run Radiant Docker image! {}", e)));
        }
    }

    let mut mapper = VolumeMapper::new();

    // Remap all file paths to container paths
    let library_container = mapper.remap_file(&config.library)?;
    let fasta_container = mapper.remap_file(&config.fasta)?;

    let config_container = if let Some(ref cfg) = config.config {
        if !cfg.is_empty() {
            Some(mapper.remap_file(cfg)?)
        } else {
            None
        }
    } else {
        None
    };

    let results_dir_container = if let Some(ref dir) = config.results_dir {
        if !dir.is_empty() {
            Some(mapper.remap_dir(dir)?)
        } else {
            None
        }
    } else {
        None
    };

    let mzml_files_container: Vec<String> = config
        .mzml_files
        .iter()
        .map(|f| mapper.remap_file(f))
        .collect::<io::Result<Vec<_>>>()?;

    // Build command arguments with remapped paths
    let mut args = vec![
        "radiant_fulcrum".to_string(),
        "-v".to_string(),
        "--library".to_string(), library_container,
        "--fasta".to_string(), fasta_container,
        "--fdr-thresh".to_string(), config.fdr_thresh,
        "--threads".to_string(), config.threads,
    ];

    if let Some(cfg) = config_container {
        args.push("--config".to_string());
        args.push(cfg);
    }

    match config.search_mode {
        SearchMode::Mbr => args.push("--mbr".to_string()),
        SearchMode::LibraryFree => args.push("--no-mbr".to_string()),
    }

    if let Some(dir) = results_dir_container {
        args.push("--results-dir".to_string());
        args.push(dir);
    }

    for file in mzml_files_container {
        args.push(file);
    }

    let mut cmd = Command::new("docker");
    cmd.arg("run").arg("--rm");

    for vol_arg in mapper.volume_args() {
        cmd.arg(vol_arg);
    }

    for (host_dir, container_dir) in mapper.mounts {
        on_output(&format!("Mounting folder {} -> {}", host_dir.display(), container_dir.display()));
    }

    on_output(&format!("Running {args:?}"));

    cmd.arg(img).args(&args);
    if dry_run {
        let mut command_parts = vec![cmd.get_program().to_string_lossy().into_owned()];
        command_parts.extend(cmd.get_args().map(|arg| arg.to_string_lossy().into_owned()));

        on_output("[dry-run] Enabled via --dry-run");
        on_output(&format!("[dry-run] docker command: {command_parts:?}"));
        on_output("[dry-run] Skipping Docker execution");

        thread::sleep(std::time::Duration::from_secs(5));
        return Ok(0);
    }

    let timestamp = Local::now().format("%Y-%m-%d-%H%M%S").to_string();
    let log_filename = format!("radiant-fulcrum-{}.log", timestamp);

    // If no results directory is configured, the CLI will create one in the
    // current working dir; we can just write our log to "." to keep things simple.
    let log_dir = config.results_dir.as_ref()
        .filter(|dir| !dir.is_empty())
        .map(|s| s.as_str())
        .unwrap_or(".");

    let mut log_file = fs::create_dir_all(log_dir)
        .ok()
        .and_then(|_| File::create(Path::new(log_dir).join(&log_filename)).ok());


    run_command(&mut on_output, &mut cmd, &mut log_file)
}

fn pull_image<F>(img: &String, mut on_output: F) -> Result<i32, io::Error>
where
    F: FnMut(&str),
{
    on_output(format!("Checking for updates of Docker image tag {}", img).as_str());

    // Check that the image can be run, and get the Radiant version
    let mut update_check = Command::new("docker");
    update_check.args(vec![
        "pull".to_string(),
        img.to_owned(),
    ]);

    run_command(&mut on_output, &mut update_check, &mut None)
}

fn run_command<F>(on_output: &mut F, cmd: &mut Command, log_file: &mut Option<File>) -> Result<i32, io::Error>
where
    F: FnMut(&str)
{
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Use a channel to collect output from both stdout and stderr concurrently
    let (tx, rx) = mpsc::channel::<String>();

    // Spawn thread for stdout
    let tx_stdout = tx.clone();
    let stdout_handle = stdout.map(|stdout| {
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx_stdout.send(line);
                }
            }
        })
    });

    // Spawn thread for stderr
    let stderr_handle = stderr.map(|stderr| {
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx.send(format!("[stderr] {}", line));
                }
            }
        })
    });

    // Receive and output lines as they come in from either stream
    for line in rx {
        on_output(&line);
        if let Some(ref mut file) = log_file {
            let _ = writeln!(file, "{}", line);
        }
    }

    // Wait for reader threads to finish
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    let status = child.wait()?;
    Ok(status.code().unwrap_or(-1))
}
