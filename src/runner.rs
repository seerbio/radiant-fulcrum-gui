use std::process::{Command, Stdio};
use std::io::{self, BufRead, BufReader};

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
}

pub fn run_pythia_scry<F>(config: RunConfig, mut on_output: F) -> io::Result<i32>
where
    F: FnMut(&str),
{
    let mut args = vec![
        "--library".to_string(), config.library,
        "--fasta".to_string(), config.fasta,
        "--fdr-thresh".to_string(), config.fdr_thresh,
        "--threads".to_string(), config.threads,
    ];

    if let Some(cfg) = config.config {
        if !cfg.is_empty() {
            args.push("--config".to_string());
            args.push(cfg);
        }
    }

    match config.search_mode {
        SearchMode::Mbr => args.push("--mbr".to_string()),
        SearchMode::LibraryFree => args.push("--no-mbr".to_string()),
    }

    if let Some(dir) = config.results_dir {
        if !dir.is_empty() {
            args.push("--results-dir".to_string());
            args.push(dir);
        }
    }

    for file in config.mzml_files {
        args.push(file);
    }

    let mut child = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/data", std::env::current_dir()?.display()))
        .arg("pythia-scry-cli")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                on_output(&line);
            }
        }
    }

    if let Some(stderr) = stderr {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                on_output(&format!("[stderr] {}", line));
            }
        }
    }

    let status = child.wait()?;
    Ok(status.code().unwrap_or(-1))
}
