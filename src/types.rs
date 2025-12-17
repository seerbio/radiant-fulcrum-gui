use serde::{Deserialize, Serialize};

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
    pub img: Option<String>,
}

