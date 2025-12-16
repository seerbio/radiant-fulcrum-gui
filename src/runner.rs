use std::process::{Command, Stdio};
use std::io::{self, BufRead, BufReader};
use std::sync::mpsc;
use std::thread;

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

pub fn run_pythia_scry<F>(config: RunConfig, mut on_output: F) -> io::Result<i32>
where
    F: FnMut(&str),
{
    let mut args = vec![
        "pythia_scry".to_string(),
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

    on_output(&format!("Running {args:?}"));

    let img = config.img.unwrap_or(DEFAULT_IMG.to_string());

    let mut child = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/data", std::env::current_dir()?.display()))
        .arg(img)
        .args(&args)
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
