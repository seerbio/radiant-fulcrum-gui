use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<FileEntry>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum SearchMode {
    LibraryFree,
    Mbr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunResult {
    pub job_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobStatus {
    pub running: bool,
    pub output: String,
    pub exit_code: Option<i32>,
}

// ============================================================================
// Fullstack (Server + Web) Implementation - uses #[server] macros
// ============================================================================
#[cfg(any(feature = "server", feature = "web"))]
mod fullstack_impl {
    use super::*;
    use dioxus::prelude::*;

    #[cfg(feature = "server")]
    use std::collections::HashMap;
    #[cfg(feature = "server")]
    use std::sync::Arc;
    #[cfg(feature = "server")]
    use tokio::sync::Mutex;
    #[cfg(feature = "server")]
    use once_cell::sync::Lazy;

    #[cfg(feature = "server")]
    struct JobState {
        output: Arc<Mutex<String>>,
        exit_code: Arc<Mutex<Option<i32>>>,
        running: Arc<Mutex<bool>>,
    }

    #[cfg(feature = "server")]
    static JOBS: Lazy<Mutex<HashMap<String, JobState>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    /// List directory contents for the file browser
    #[server(ListDirectory)]
    pub async fn list_directory(path: Option<String>) -> Result<DirectoryListing, ServerFnError> {
        use std::path::PathBuf;

        let path = path.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .to_string_lossy()
                .to_string()
        });

        let path_buf = PathBuf::from(&path);
        let canonical = path_buf.canonicalize().map_err(|e| ServerFnError::new(e.to_string()))?;

        let parent_path = canonical.parent().map(|p| p.to_string_lossy().to_string());

        let mut entries = Vec::new();

        let read_dir = std::fs::read_dir(&canonical).map_err(|e| ServerFnError::new(e.to_string()))?;

        for entry in read_dir {
            let entry = entry.map_err(|e| ServerFnError::new(e.to_string()))?;
            let metadata = entry.metadata().map_err(|e| ServerFnError::new(e.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            entries.push(FileEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
            });
        }

        // Sort: directories first, then files, alphabetically
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        Ok(DirectoryListing {
            current_path: canonical.to_string_lossy().to_string(),
            parent_path,
            entries,
        })
    }

    /// Start a pythia-scry job and return a job ID for tracking
    #[server(StartPythiaScry)]
    pub async fn start_pythia_scry(config: RunConfig) -> Result<RunResult, ServerFnError> {
        use crate::runner::{RunConfig as RunnerConfig, SearchMode as RunnerSearchMode, run_pythia_scry};

        let job_id = uuid::Uuid::new_v4().to_string();

        // Convert from serde-compatible types to runner types
        let runner_config = RunnerConfig {
            library: config.library,
            fasta: config.fasta,
            config: config.config,
            search_mode: match config.search_mode {
                SearchMode::LibraryFree => RunnerSearchMode::LibraryFree,
                SearchMode::Mbr => RunnerSearchMode::Mbr,
            },
            fdr_thresh: config.fdr_thresh,
            threads: config.threads,
            results_dir: config.results_dir,
            mzml_files: config.mzml_files,
            img: None,
        };

        // Store job state in a global registry
        let output = Arc::new(Mutex::new(String::new()));
        let exit_code = Arc::new(Mutex::new(None::<i32>));
        let running = Arc::new(Mutex::new(true));

        {
            let mut jobs = JOBS.lock().await;
            jobs.insert(job_id.clone(), JobState {
                output: output.clone(),
                exit_code: exit_code.clone(),
                running: running.clone(),
            });
        }

        let output_clone = output.clone();
        let exit_code_clone = exit_code.clone();
        let running_clone = running.clone();

        // Spawn the job in a background thread
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();

            let result = run_pythia_scry(runner_config, |line| {
                let output = output_clone.clone();
                let line = line.to_string();
                rt.block_on(async {
                    let mut output = output.lock().await;
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&line);
                });
            });

            rt.block_on(async {
                match result {
                    Ok(code) => {
                        let mut output = output_clone.lock().await;
                        output.push_str(&format!("\n--- Process exited with code {} ---", code));
                        *exit_code_clone.lock().await = Some(code);
                    }
                    Err(e) => {
                        let mut output = output_clone.lock().await;
                        output.push_str(&format!("\n--- Failed to run: {} ---", e));
                        *exit_code_clone.lock().await = Some(-1);
                    }
                }
                *running_clone.lock().await = false;
            });
        });

        Ok(RunResult { job_id })
    }

    /// Get the current status of a running job
    #[server(GetJobStatus)]
    pub async fn get_job_status(job_id: String) -> Result<JobStatus, ServerFnError> {
        let jobs = JOBS.lock().await;

        if let Some(job) = jobs.get(&job_id) {
            let output = job.output.lock().await.clone();
            let exit_code = *job.exit_code.lock().await;
            let running = *job.running.lock().await;

            Ok(JobStatus {
                running,
                output,
                exit_code,
            })
        } else {
            Err(ServerFnError::new("Job not found"))
        }
    }
}

#[cfg(any(feature = "server", feature = "web"))]
pub use fullstack_impl::*;

// ============================================================================
// Desktop Implementation - runs locally without server functions
// ============================================================================
#[cfg(all(feature = "desktop", not(feature = "server"), not(feature = "web")))]
mod desktop_impl {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use once_cell::sync::Lazy;

    struct JobState {
        output: Arc<Mutex<String>>,
        exit_code: Arc<Mutex<Option<i32>>>,
        running: Arc<Mutex<bool>>,
    }

    static JOBS: Lazy<Mutex<HashMap<String, JobState>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    /// List directory contents for the file browser (desktop - runs locally)
    pub async fn list_directory(path: Option<String>) -> Result<DirectoryListing, String> {
        use std::path::PathBuf;

        let path = path.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .to_string_lossy()
                .to_string()
        });

        let path_buf = PathBuf::from(&path);
        let canonical = path_buf.canonicalize().map_err(|e| e.to_string())?;

        let parent_path = canonical.parent().map(|p| p.to_string_lossy().to_string());

        let mut entries = Vec::new();

        let read_dir = std::fs::read_dir(&canonical).map_err(|e| e.to_string())?;

        for entry in read_dir {
            let entry = entry.map_err(|e| e.to_string())?;
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            entries.push(FileEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
            });
        }

        // Sort: directories first, then files, alphabetically
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        Ok(DirectoryListing {
            current_path: canonical.to_string_lossy().to_string(),
            parent_path,
            entries,
        })
    }

    /// Start a pythia-scry job (desktop - runs locally)
    pub async fn start_pythia_scry(config: RunConfig) -> Result<RunResult, String> {
        use crate::runner::{RunConfig as RunnerConfig, SearchMode as RunnerSearchMode, run_pythia_scry};

        let job_id = uuid::Uuid::new_v4().to_string();

        // Convert from serde-compatible types to runner types
        let runner_config = RunnerConfig {
            library: config.library,
            fasta: config.fasta,
            config: config.config,
            search_mode: match config.search_mode {
                SearchMode::LibraryFree => RunnerSearchMode::LibraryFree,
                SearchMode::Mbr => RunnerSearchMode::Mbr,
            },
            fdr_thresh: config.fdr_thresh,
            threads: config.threads,
            results_dir: config.results_dir,
            mzml_files: config.mzml_files,
            img: None,
        };

        // Store job state in a global registry
        let output = Arc::new(Mutex::new(String::new()));
        let exit_code = Arc::new(Mutex::new(None::<i32>));
        let running = Arc::new(Mutex::new(true));

        {
            let mut jobs = JOBS.lock().await;
            jobs.insert(job_id.clone(), JobState {
                output: output.clone(),
                exit_code: exit_code.clone(),
                running: running.clone(),
            });
        }

        let output_clone = output.clone();
        let exit_code_clone = exit_code.clone();
        let running_clone = running.clone();

        // Spawn the job in a background thread
        std::thread::spawn(move || {
            let result = run_pythia_scry(runner_config, |line| {
                let output = output_clone.clone();
                let line = line.to_string();
                // Use block_on for sync context
                futures_lite::future::block_on(async {
                    let mut output = output.lock().await;
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&line);
                });
            });

            futures_lite::future::block_on(async {
                match result {
                    Ok(code) => {
                        let mut output = output_clone.lock().await;
                        output.push_str(&format!("\n--- Process exited with code {} ---", code));
                        *exit_code_clone.lock().await = Some(code);
                    }
                    Err(e) => {
                        let mut output = output_clone.lock().await;
                        output.push_str(&format!("\n--- Failed to run: {} ---", e));
                        *exit_code_clone.lock().await = Some(-1);
                    }
                }
                *running_clone.lock().await = false;
            });
        });

        Ok(RunResult { job_id })
    }

    /// Get the current status of a running job (desktop - runs locally)
    pub async fn get_job_status(job_id: String) -> Result<JobStatus, String> {
        let jobs = JOBS.lock().await;

        if let Some(job) = jobs.get(&job_id) {
            let output = job.output.lock().await.clone();
            let exit_code = *job.exit_code.lock().await;
            let running = *job.running.lock().await;

            Ok(JobStatus {
                running,
                output,
                exit_code,
            })
        } else {
            Err("Job not found".to_string())
        }
    }
}

#[cfg(all(feature = "desktop", not(feature = "server"), not(feature = "web")))]
pub use desktop_impl::*;
