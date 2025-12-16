use std::process::{Command, Stdio};
use std::io::{self, BufRead, BufReader};
use std::sync::mpsc;
use std::thread;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

const DEFAULT_IMG: &'static str = "718843040700.dkr.ecr.us-west-2.amazonaws.com/seer/pythia-scry:latest";

#[derive(Clone, Copy, PartialEq)]
pub enum SearchMode {
    LibraryFree,
    Mbr,
}

pub struct RunConfig {
    pub library: String,
    pub fasta: String,
    pub config: Option<String>,
    pub search_mode: SearchMode,
    pub fdr_thresh: String,
    pub threads: String,
    pub results_dir: Option<String>,
    pub mzml_files: Vec<String>,
    pub img: Option<String>,
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

    /// Registers a file path and returns the remapped container path
    fn remap_file(&mut self, host_path: &str) -> io::Result<String> {
        let path = Path::new(host_path);
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let abs_path = abs_path.canonicalize().unwrap_or(abs_path);

        let parent = abs_path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Path has no parent directory")
        })?;

        let mount_point = if let Some(existing) = self.mounts.get(parent) {
            existing.clone()
        } else {
            let mount_point = PathBuf::from(format!("/data{}", self.mount_counter));
            self.mount_counter += 1;
            self.mounts.insert(parent.to_path_buf(), mount_point.clone());
            mount_point
        };

        let file_name = abs_path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Path has no file name")
        })?;

        Ok(mount_point.join(file_name).to_string_lossy().to_string())
    }

    /// Registers a directory path and returns the remapped container path
    fn remap_dir(&mut self, host_path: &str) -> io::Result<String> {
        let path = Path::new(host_path);
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let abs_path = abs_path.canonicalize().unwrap_or(abs_path);

        let mount_point = if let Some(existing) = self.mounts.get(&abs_path) {
            existing.clone()
        } else {
            let mount_point = PathBuf::from(format!("/data{}", self.mount_counter));
            self.mount_counter += 1;
            self.mounts.insert(abs_path, mount_point.clone());
            mount_point
        };

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

pub fn run_pythia_scry<F>(config: RunConfig, mut on_output: F) -> io::Result<i32>
where
    F: FnMut(&str),
{
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
        "pythia_scry".to_string(),
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

    on_output(&format!("Running {args:?}"));

    let img = config.img.unwrap_or(DEFAULT_IMG.to_string());

    let mut cmd = Command::new("docker");
    cmd.arg("run").arg("--rm");

    for vol_arg in mapper.volume_args() {
        cmd.arg(vol_arg);
    }

    cmd.arg(img).args(&args);

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
