use serde::{Deserialize, Serialize};
use crate::types::{RunConfig, SearchMode};

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

#[cfg(not(target_arch = "wasm32"))]
mod shared_impl {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use once_cell::sync::Lazy;
    use std::path::PathBuf;

    pub(super) struct JobState {
        pub output: Arc<Mutex<String>>,
        pub exit_code: Arc<Mutex<Option<i32>>>,
        pub running: Arc<Mutex<bool>>,
    }

    pub(super) static JOBS: Lazy<Mutex<HashMap<String, JobState>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));

    pub fn list_directory_impl(path: Option<String>) -> Result<DirectoryListing, String> {
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

            if name.starts_with('.') {
                continue;
            }

            entries.push(FileEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
            });
        }

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

    pub async fn start_pythia_scry_impl<F>(
        config: RunConfig,
        spawn_fn: F,
    ) -> Result<RunResult, String>
    where
        F: FnOnce(Box<dyn FnOnce() + Send>) + Send + 'static,
    {
        use crate::runner::run_pythia_scry;

        let job_id = uuid::Uuid::new_v4().to_string();

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

        spawn_fn(Box::new(move || {
            let result = run_pythia_scry(config, |line| {
                let output = output_clone.clone();
                let line = line.to_string();
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
        }));

        Ok(RunResult { job_id })
    }

    pub async fn get_job_status_impl(job_id: String) -> Result<JobStatus, String> {
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

// ============================================================================
// Fullstack (Server + Web) Implementation - uses #[server] macros
// ============================================================================
#[cfg(any(feature = "server", feature = "web"))]
mod fullstack_impl {
    use super::*;
    use dioxus::prelude::*;

    #[server(ListDirectory)]
    pub async fn list_directory(path: Option<String>) -> Result<DirectoryListing, ServerFnError> {
        shared_impl::list_directory_impl(path).map_err(ServerFnError::new)
    }

    #[server(StartPythiaScry)]
    pub async fn start_pythia_scry(config: RunConfig) -> Result<RunResult, ServerFnError> {
        shared_impl::start_pythia_scry_impl(config, |task| {
            tokio::task::spawn_blocking(task);
        })
        .await
        .map_err(ServerFnError::new)
    }

    #[server(GetJobStatus)]
    pub async fn get_job_status(job_id: String) -> Result<JobStatus, ServerFnError> {
        shared_impl::get_job_status_impl(job_id).await.map_err(ServerFnError::new)
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

    pub async fn list_directory(path: Option<String>) -> Result<DirectoryListing, String> {
        shared_impl::list_directory_impl(path)
    }

    pub async fn start_pythia_scry(config: RunConfig) -> Result<RunResult, String> {
        shared_impl::start_pythia_scry_impl(config, |task| {
            std::thread::spawn(task);
        })
        .await
    }

    pub async fn get_job_status(job_id: String) -> Result<JobStatus, String> {
        shared_impl::get_job_status_impl(job_id).await
    }
}

#[cfg(all(feature = "desktop", not(feature = "server"), not(feature = "web")))]
pub use desktop_impl::*;
