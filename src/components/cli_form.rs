use dioxus::prelude::*;
use crate::types::{SearchMode, RunConfig};
use crate::storage;
use std::path::Path;

use crate::server_fns::{start_radiant_fulcrum, get_job_status};

use super::collapsible::{Collapsible, CollapsibleContent, CollapsibleTrigger};
#[cfg(not(feature = "desktop"))]
use super::file_browser::{FileBrowser, FileBrowserMode};
use super::form_primitives::{FilePathField, MultiFileField};
use super::radio_group::{RadioGroup, RadioItem};
use super::scroll_area::ScrollArea;
use super::switch::{Switch, SwitchThumb};

pub static LAST_DIRECTORY: GlobalSignal<Option<String>> = Signal::global(|| None);
const PANEL_TITLE_CLASS: &str = "text-xl font-bold mb-2 dark:text-gray-100";
const FIELD_LABEL_CLASS: &str = "text-sm font-medium dark:text-gray-200";
const FIELD_GROUP_CLASS: &str = "flex flex-col gap-1";
const NUMERIC_INPUT_CLASS: &str = "p-2 border rounded dark:bg-gray-900 dark:text-gray-100";

async fn sleep_ms(ms: u64) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::TimeoutFuture::new(ms as u32).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
}

pub fn update_last_dir(path: &str) {
    *LAST_DIRECTORY.write() = Some(path.to_string());
}

fn get_filename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

#[cfg(feature = "desktop")]
fn update_last_dir_from_file(path: &str) {
    use std::path::Path;
    if let Some(parent) = Path::new(path).parent() {
        update_last_dir(&parent.display().to_string());
    }
}

#[cfg(feature = "desktop")]
fn apply_last_dir(dialog: rfd::AsyncFileDialog) -> rfd::AsyncFileDialog {
    if let Some(ref dir) = *LAST_DIRECTORY.read() {
        dialog.set_directory(dir)
    } else {
        dialog
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BrowserTarget {
    Library,
    Fasta,
    Config,
    ResultsDir,
    MzmlFiles,
}

fn cli_form_layout(file_browser_element: Element, params_panel: Element, output_panel: Element) -> Element {
    rsx! {
        // File browser modal (web/server only)
        {file_browser_element}

        div { class: "flex gap-6 p-6 h-full overflow-hidden",
            {params_panel}
            {output_panel}
        }
    }
}

fn output_panel(output: String) -> Element {
    rsx! {
        // Right column - Log/Console Output (2/3 width)
        div { class: "w-2/3 flex flex-col min-h-0",
            h2 { class: PANEL_TITLE_CLASS, "Console Output" }
            ScrollArea { class: "flex-1 p-4 bg-gray-100 dark:bg-gray-900 rounded shadow text-xs font-mono dark:text-gray-100 whitespace-pre-wrap overflow-auto min-h-0".to_string(),
                "{output}"
            }
        }
    }
}

#[derive(Clone, Copy)]
struct CliFormState {
    library: Signal<String>,
    fasta: Signal<String>,
    config: Signal<String>,
    search_mode: Signal<SearchMode>,
    fdr_thresh: Signal<String>,
    threads: Signal<String>,
    results_dir: Signal<String>,
    mzml_files: Signal<Vec<String>>,
    output: Signal<String>,
    running: Signal<bool>,
    job_id: Signal<Option<String>>,
    show_advanced: Signal<bool>,
    check_image_updates: Signal<bool>,
    #[cfg(not(feature = "desktop"))]
    show_browser: Signal<Option<BrowserTarget>>,
}

impl CliFormState {
    fn save_last_files(self) {
        let last_files = storage::LastFiles {
            library: {
                let lib = self.library.read().clone();
                if lib.is_empty() { None } else { Some(lib) }
            },
            fasta: {
                let fas = self.fasta.read().clone();
                if fas.is_empty() { None } else { Some(fas) }
            },
            config: {
                let cfg = self.config.read().clone();
                if cfg.is_empty() { None } else { Some(cfg) }
            },
        };
        storage::save(&last_files);
    }

    fn clear_library(mut self) {
        self.library.set(String::new());
        self.save_last_files();
    }

    fn clear_fasta(mut self) {
        self.fasta.set(String::new());
        self.save_last_files();
    }

    fn clear_config(mut self) {
        self.config.set(String::new());
        self.save_last_files();
    }

    fn clear_results_dir(mut self) {
        self.results_dir.set(String::new());
    }

    fn add_mzml_files(mut self, new_paths: Vec<String>) {
        let mut current_files = self.mzml_files.read().clone();
        for path in new_paths {
            if !current_files.contains(&path) {
                current_files.push(path);
            }
        }
        self.mzml_files.set(current_files);
    }

    fn handle_file_selection(mut self, target: BrowserTarget, paths: Vec<String>) {
        match target {
            BrowserTarget::Library => if let Some(path) = paths.first() {
                self.library.set(path.clone());
                self.save_last_files();
            },
            BrowserTarget::Fasta => if let Some(path) = paths.first() {
                self.fasta.set(path.clone());
                self.save_last_files();
            },
            BrowserTarget::Config => if let Some(path) = paths.first() {
                self.config.set(path.clone());
                self.save_last_files();
            },
            BrowserTarget::ResultsDir => if let Some(path) = paths.first() {
                self.results_dir.set(path.clone());
            },
            BrowserTarget::MzmlFiles => self.add_mzml_files(paths),
        }
    }

    fn run_config(self) -> RunConfig {
        RunConfig {
            library: self.library.read().clone(),
            fasta: self.fasta.read().clone(),
            config: {
                let c = self.config.read().clone();
                if c.is_empty() { None } else { Some(c) }
            },
            search_mode: *self.search_mode.read(),
            fdr_thresh: self.fdr_thresh.read().clone(),
            threads: self.threads.read().clone(),
            results_dir: {
                let r = self.results_dir.read().clone();
                if r.is_empty() { None } else { Some(r) }
            },
            mzml_files: self.mzml_files.read().clone(),
            img: None,
            check_image_updates: *self.check_image_updates.read(),
        }
    }

    fn missing_required_fields(self) -> Vec<&'static str> {
        let mut missing = Vec::new();

        if self.library.read().trim().is_empty() {
            missing.push("Library");
        }
        if self.fasta.read().trim().is_empty() {
            missing.push("FASTA");
        }
        if self.results_dir.read().trim().is_empty() {
            missing.push("Results Directory");
        }
        if self.mzml_files.read().is_empty() {
            missing.push("mzML Files");
        }

        missing
    }
}

fn use_cli_form_state() -> CliFormState {
    CliFormState {
        library: use_signal(String::new),
        fasta: use_signal(String::new),
        config: use_signal(String::new),
        search_mode: use_signal(|| SearchMode::LibraryFree),
        fdr_thresh: use_signal(|| "0.01".to_string()),
        threads: use_signal(|| "0".to_string()),
        results_dir: use_signal(String::new),
        mzml_files: use_signal(Vec::<String>::new),
        output: use_signal(String::new),
        running: use_signal(|| false),
        job_id: use_signal(|| None::<String>),
        show_advanced: use_signal(|| false),
        check_image_updates: use_signal(|| false),
        #[cfg(not(feature = "desktop"))]
        show_browser: use_signal(|| None::<BrowserTarget>),
    }
}

#[component]
pub fn CliForm() -> Element {
    let state = use_cli_form_state();
    let mut library = state.library;
    let mut fasta = state.fasta;
    let mut config = state.config;
    let mut search_mode = state.search_mode;
    let mut search_mode_value = use_signal(|| Some("library_free".to_string()));
    let mut fdr_thresh = state.fdr_thresh;
    let mut threads = state.threads;
    let mut results_dir = state.results_dir;
    let mut mzml_files = state.mzml_files;
    let mut output = state.output;
    let mut running = state.running;
    let mut job_id = state.job_id;
    let mut show_advanced = state.show_advanced;
    let mut check_image_updates = state.check_image_updates;

    #[cfg(not(feature = "desktop"))]
    let mut show_browser = state.show_browser;

    use_effect(move || {
        let last_files = storage::load();
        if let Some(lib) = last_files.library {
            library.set(lib);
        }
        if let Some(fas) = last_files.fasta {
            fasta.set(fas);
        }
        if let Some(cfg) = last_files.config {
            config.set(cfg);
        }
    });

    use_effect(move || {
        let next = Some(match *search_mode.read() {
            SearchMode::LibraryFree => "library_free".to_string(),
            SearchMode::Mbr => "mbr".to_string(),
        });
        if *search_mode_value.read() != next {
            search_mode_value.set(next);
        }
    });

    let clear_library = move |_| state.clear_library();
    let clear_fasta = move |_| state.clear_fasta();
    let clear_config = move |_| state.clear_config();
    let clear_results_dir = move |_| state.clear_results_dir();

    let handle_file_selection = move |target: BrowserTarget, paths: Vec<String>| {
        state.handle_file_selection(target, paths);
    };

    #[cfg(feature = "desktop")]
    let pick_file = move |target: BrowserTarget| {
        spawn(async move {
            match target {
                BrowserTarget::Library => {
                    let dialog = apply_last_dir(
                        rfd::AsyncFileDialog::new()
                            .add_filter("Library", &["tsv", "parquet", "speclib"])
                    );
                    if let Some(path) = dialog.pick_file().await {
                        let path_str = path.path().display().to_string();
                        update_last_dir_from_file(&path_str);
                        handle_file_selection(target, vec![path_str]);
                    }
                }
                BrowserTarget::Fasta => {
                    let dialog = apply_last_dir(
                        rfd::AsyncFileDialog::new()
                            .add_filter("FASTA", &["fasta", "fas"])
                    );
                    if let Some(path) = dialog.pick_file().await {
                        let path_str = path.path().display().to_string();
                        update_last_dir_from_file(&path_str);
                        handle_file_selection(target, vec![path_str]);
                    }
                }
                BrowserTarget::Config => {
                    let dialog = apply_last_dir(
                        rfd::AsyncFileDialog::new()
                            .add_filter("Radiant Config", &["radiantConfig", "toml", "pythiaConfig"])
                    );
                    if let Some(path) = dialog.pick_file().await {
                        let path_str = path.path().display().to_string();
                        update_last_dir_from_file(&path_str);
                        handle_file_selection(target, vec![path_str]);
                    }
                }
                BrowserTarget::MzmlFiles => {
                    let dialog = apply_last_dir(
                        rfd::AsyncFileDialog::new()
                            .add_filter("mzML", &["mzML", "mzml"])
                    );
                    if let Some(files) = dialog.pick_files().await {
                        let new_paths: Vec<String> = files.iter().map(|f| f.path().display().to_string()).collect();
                        if let Some(first) = new_paths.first() {
                            update_last_dir_from_file(first);
                        }
                        handle_file_selection(target, new_paths);
                    }
                }
                BrowserTarget::ResultsDir => {
                    let dialog = apply_last_dir(rfd::AsyncFileDialog::new());
                    if let Some(path) = dialog.pick_folder().await {
                        let path_str = path.path().display().to_string();
                        update_last_dir(&path_str);
                        handle_file_selection(target, vec![path_str]);
                    }
                }
            }
        });
    };

    #[cfg(feature = "desktop")]
    let pick_library = move |_| pick_file(BrowserTarget::Library);

    #[cfg(feature = "desktop")]
    let pick_fasta = move |_| pick_file(BrowserTarget::Fasta);

    #[cfg(feature = "desktop")]
    let pick_config = move |_| pick_file(BrowserTarget::Config);

    #[cfg(feature = "desktop")]
    let pick_mzml = move |_| pick_file(BrowserTarget::MzmlFiles);

    #[cfg(feature = "desktop")]
    let pick_results_dir = move |_| pick_file(BrowserTarget::ResultsDir);

    #[cfg(not(feature = "desktop"))]
    let pick_library = move |_| show_browser.set(Some(BrowserTarget::Library));

    #[cfg(not(feature = "desktop"))]
    let pick_fasta = move |_| show_browser.set(Some(BrowserTarget::Fasta));

    #[cfg(not(feature = "desktop"))]
    let pick_config = move |_| show_browser.set(Some(BrowserTarget::Config));

    #[cfg(not(feature = "desktop"))]
    let pick_mzml = move |_| show_browser.set(Some(BrowserTarget::MzmlFiles));

    #[cfg(not(feature = "desktop"))]
    let pick_results_dir = move |_| show_browser.set(Some(BrowserTarget::ResultsDir));

    #[cfg(not(feature = "desktop"))]
    let on_browser_select = move |paths: Vec<String>| {
        if let Some(target) = *show_browser.read() {
            handle_file_selection(target, paths);
        }
        show_browser.set(None);
    };

    #[cfg(not(feature = "desktop"))]
    let on_browser_cancel = move |_| show_browser.set(None);

    // Poll for job status when running
    use_effect(move || {
        let job_id_val = job_id.read().clone();
        if let Some(id) = job_id_val {
            spawn(async move {
                loop {
                    match get_job_status(id.clone()).await {
                        Ok(status) => {
                            output.set(status.output);
                            if !status.running {
                                running.set(false);
                                job_id.set(None);
                                break;
                            }
                        }
                        Err(e) => {
                            output.set(format!("Error getting job status: {}", e));
                            running.set(false);
                            job_id.set(None);
                            break;
                        }
                    }
                    sleep_ms(500).await;
                }
            });
        }
    });

    let on_submit = move |evt: Event<FormData>| {
        evt.prevent_default();
        if *running.read() { return; }

        let missing = state.missing_required_fields();
        if !missing.is_empty() {
            output.set(format!(
                "Cannot start run. Missing required field(s):\n- {}",
                missing.join("\n- ")
            ));
            return;
        }

        running.set(true);
        output.set("Starting job...".to_string());

        let run_config = state.run_config();

        spawn(async move {
            match start_radiant_fulcrum(run_config).await {
                Ok(result) => {
                    job_id.set(Some(result.job_id));
                }
                Err(e) => {
                    output.set(format!("Failed to start job: {}", e));
                    running.set(false);
                }
            }
        });
    };

    // Build the file browser element for non-desktop builds
    #[cfg(not(feature = "desktop"))]
    let file_browser_element = {
        let browser_mode = match *show_browser.read() {
            Some(BrowserTarget::Library) => Some(FileBrowserMode::File {
                extensions: vec!["tsv".into(), "parquet".into(), "speclib".into()],
            }),
            Some(BrowserTarget::Fasta) => Some(FileBrowserMode::File {
                extensions: vec!["fasta".into(), "fas".into()],
            }),
            Some(BrowserTarget::Config) => Some(FileBrowserMode::File {
                extensions: vec!["radiantConfig".into(), "toml".into(), "pythiaConfig".into()],
            }),
            Some(BrowserTarget::ResultsDir) => Some(FileBrowserMode::Directory),
            Some(BrowserTarget::MzmlFiles) => Some(FileBrowserMode::MultiFile {
                extensions: vec!["mzML".into(), "mzml".into()],
            }),
            None => None,
        };

        if let Some(mode) = browser_mode {
            rsx! {
                FileBrowser {
                    mode: mode,
                    on_select: on_browser_select,
                    on_cancel: on_browser_cancel,
                }
            }
        } else {
            rsx! {}
        }
    };

    #[cfg(feature = "desktop")]
    let file_browser_element = rsx! {};
    let has_missing_required = !state.missing_required_fields().is_empty();

    let params_panel = rsx! {
        // Left column - Parameters (1/3 width)
        form { class: "w-1/3 p-6 bg-white dark:bg-gray-800 rounded shadow flex flex-col gap-4 overflow-y-auto",
            novalidate: true,
            onsubmit: on_submit,
            h2 { class: PANEL_TITLE_CLASS, "Run Radiant+Fulcrum Workflow" }

            div { class: FIELD_GROUP_CLASS,
                label { class: FIELD_LABEL_CLASS, "Search Mode" }
                RadioGroup {
                    class: "flex gap-4 dark:text-gray-200",
                    horizontal: true,
                    name: "search_mode",
                    value: search_mode_value,
                    on_value_change: move |value: String| {
                        search_mode_value.set(Some(value.clone()));
                        match value.as_str() {
                            "mbr" => search_mode.set(SearchMode::Mbr),
                            _ => search_mode.set(SearchMode::LibraryFree),
                        }
                    },
                    RadioItem {
                        value: "library_free".to_string(),
                        index: 0usize,
                        "Library-free"
                    }
                    RadioItem {
                        value: "mbr".to_string(),
                        index: 1usize,
                        "Match Between Runs (MBR)"
                    }
                }
            }

            MultiFileField {
                title: "mzML Files".to_string(),
                files: mzml_files.read().clone(),
                on_remove: move |idx| {
                    let mut files = mzml_files.read().clone();
                    files.remove(idx);
                    mzml_files.set(files);
                },
                on_browse: pick_mzml,
            }

            FilePathField {
                title: "Library".to_string(),
                placeholder: "Select library file...".to_string(),
                value: get_filename(&library.read()).to_string(),
                full_path: library.read().clone(),
                required: true,
                oninput: move |value| library.set(value),
                onbrowse: pick_library,
                onclear: clear_library,
            }

            FilePathField {
                title: "FASTA".to_string(),
                placeholder: "Select FASTA file...".to_string(),
                value: get_filename(&fasta.read()).to_string(),
                full_path: fasta.read().clone(),
                required: true,
                oninput: move |value| fasta.set(value),
                onbrowse: pick_fasta,
                onclear: clear_fasta,
            }

            FilePathField {
                title: "Results Directory".to_string(),
                placeholder: "Select output directory...".to_string(),
                value: get_filename(&results_dir.read()).to_string(),
                full_path: results_dir.read().clone(),
                required: true,
                oninput: move |value| results_dir.set(value),
                onbrowse: pick_results_dir,
                onclear: clear_results_dir,
            }

            div { class: "flex flex-col gap-2 mt-2",
                Collapsible {
                    open: Some(*show_advanced.read()),
                    on_open_change: move |is_open| show_advanced.set(is_open),
                    CollapsibleTrigger {
                        class: "text-left text-sm font-medium dark:text-gray-200 hover:text-blue-600 dark:hover:text-blue-400",
                        if *show_advanced.read() { "▼ Advanced" } else { "▶ Advanced" }
                    }
                    CollapsibleContent {
                        class: "pt-1",
                            div { class: "flex flex-col gap-4 pl-4 border-l-2 border-gray-300 dark:border-gray-600",
                            div { class: FIELD_GROUP_CLASS,
                                label { class: FIELD_LABEL_CLASS, "FDR Threshold" }
                                input { class: NUMERIC_INPUT_CLASS,
                                    r#type: "number", step: "0.001", min: "0", max: "1", value: "{fdr_thresh}",
                                    oninput: move |e| fdr_thresh.set(e.value().clone()) }
                            }

                            FilePathField {
                                title: "Radiant Config (optional)".to_string(),
                                placeholder: "Select .radiantConfig file...".to_string(),
                                value: get_filename(&config.read()).to_string(),
                                full_path: config.read().clone(),
                                oninput: move |value| config.set(value),
                                onbrowse: pick_config,
                                onclear: clear_config,
                            }

                            div { class: FIELD_GROUP_CLASS,
                                label { class: FIELD_LABEL_CLASS, "Threads (0 = auto)" }
                                input { class: NUMERIC_INPUT_CLASS,
                                    r#type: "number", min: "0", value: "{threads}",
                                    oninput: move |e| threads.set(e.value().clone()) }
                            }

                            div { class: "flex flex-col gap-1",
                                label { class: "flex items-center gap-2 text-sm font-medium dark:text-gray-200 cursor-pointer",
                                    Switch {
                                        checked: Some(*check_image_updates.read()),
                                        on_checked_change: move |is_checked| check_image_updates.set(is_checked),
                                        aria_label: "Check for Docker image updates",
                                        SwitchThumb {}
                                    }
                                    "Check for Docker image updates"
                                }
                            }
                        }
                    }
                }
            }

            button {
                class: format!(
                    "mt-2 py-2 px-4 text-white font-semibold rounded {}",
                    if *running.read() {
                        "bg-gray-500 opacity-70 cursor-not-allowed"
                    } else if has_missing_required {
                        "bg-blue-500 hover:bg-blue-600 cursor-pointer"
                    } else {
                        "bg-blue-600 hover:bg-blue-700"
                    }
                ),
                r#type: "submit",
                disabled: *running.read(),
                aria_disabled: if *running.read() { "true" } else { "false" },
                title: if *running.read() {
                    "Workflow is currently running"
                } else if has_missing_required {
                    "Click to see what is missing"
                } else {
                    "Run workflow"
                },
                if *running.read() {
                    "Running..."
                } else {
                    "Run"
                }
            }
        }
    };

    let output_text = output.read().clone();
    let output_panel = output_panel(output_text);
    cli_form_layout(file_browser_element, params_panel, output_panel)
}
