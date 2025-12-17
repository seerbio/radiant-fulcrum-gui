use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LastFiles {
    pub library: Option<String>,
    pub fasta: Option<String>,
    pub config: Option<String>,
}

impl Default for LastFiles {
    fn default() -> Self {
        Self {
            library: None,
            fasta: None,
            config: None,
        }
    }
}

#[cfg(feature = "desktop")]
mod desktop_impl {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("pythia-scry-gui").join("last_files.toml"))
    }

    pub fn load() -> LastFiles {
        config_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| toml::from_str::<LastFiles>(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(last_files: &LastFiles) {
        if let Some(path) = config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(content) = toml::to_string_pretty(last_files) {
                let _ = fs::write(path, content);
            }
        }
    }
}

#[cfg(all(target_arch = "wasm32", not(feature = "desktop")))]
mod web_impl {
    use super::*;

    const STORAGE_KEY: &str = "pythia_scry_last_files";

    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load() -> LastFiles {
        local_storage()
            .and_then(|storage| storage.get_item(STORAGE_KEY).ok()?)
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(last_files: &LastFiles) {
        if let Some(storage) = local_storage() {
            if let Ok(json) = serde_json::to_string(last_files) {
                let _ = storage.set_item(STORAGE_KEY, &json);
            }
        }
    }
}

#[cfg(all(not(feature = "desktop"), not(target_arch = "wasm32")))]
mod server_impl {
    use super::*;

    pub fn load() -> LastFiles {
        LastFiles::default()
    }

    pub fn save(_last_files: &LastFiles) {}
}

#[cfg(feature = "desktop")]
pub use desktop_impl::{load, save};

#[cfg(all(target_arch = "wasm32", not(feature = "desktop")))]
pub use web_impl::{load, save};

#[cfg(all(not(feature = "desktop"), not(target_arch = "wasm32")))]
pub use server_impl::{load, save};
