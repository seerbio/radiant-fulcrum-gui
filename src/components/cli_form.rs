use dioxus::prelude::*;
use crate::types::{SearchMode, RunConfig};
use crate::storage;
use std::path::Path;

use crate::server_fns::{start_radiant_fulcrum, get_job_status};

#[cfg(not(feature = "desktop"))]
use super::file_browser::{FileBrowser, FileBrowserMode};

pub static LAST_DIRECTORY: GlobalSignal<Option<String>> = Signal::global(|| None);

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
            h2 { class: "text-xl font-bold mb-2 dark:text-gray-100", "Console Output" }
            div { class: "flex-1 p-4 bg-gray-100 dark:bg-gray-900 rounded shadow text-xs font-mono dark:text-gray-100 whitespace-pre-wrap overflow-auto min-h-0",
                "{output}"
            }
        }
    }
}

#[component]
pub fn CliForm() -> Element {
    let mut library = use_signal(String::new);
    let mut fasta = use_signal(String::new);
    let mut config = use_signal(String::new);
    let mut search_mode = use_signal(|| SearchMode::LibraryFree);
    let mut fdr_thresh = use_signal(|| "0.01".to_string());
    let mut threads = use_signal(|| "0".to_string());
    let mut results_dir = use_signal(String::new);
    let mut mzml_files = use_signal(Vec::<String>::new);
    let mut output = use_signal(String::new);
    let mut running = use_signal(|| false);
    let mut job_id = use_signal(|| None::<String>);
    let mut show_advanced = use_signal(|| false);
    let mut check_image_updates = use_signal(|| false);

    #[cfg(not(feature = "desktop"))]
    let mut show_browser = use_signal(|| None::<BrowserTarget>);

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

    let save_last_files = move || {
        let last_files = storage::LastFiles {
            library: {
                let lib = library.read().clone();
                if lib.is_empty() { None } else { Some(lib) }
            },
            fasta: {
                let fas = fasta.read().clone();
                if fas.is_empty() { None } else { Some(fas) }
            },
            config: {
                let cfg = config.read().clone();
                if cfg.is_empty() { None } else { Some(cfg) }
            },
        };
        storage::save(&last_files);
    };

    let clear_library = move |_| {
        library.set(String::new());
        save_last_files();
    };

    let clear_fasta = move |_| {
        fasta.set(String::new());
        save_last_files();
    };

    let clear_config = move |_| {
        config.set(String::new());
        save_last_files();
    };

    let mut add_mzml_files = move |new_paths: Vec<String>| {
        let mut current_files = mzml_files.read().clone();
        for path in new_paths {
            if !current_files.contains(&path) {
                current_files.push(path);
            }
        }
        mzml_files.set(current_files);
    };

    let mut handle_file_selection = move |target: BrowserTarget, paths: Vec<String>| {
        match target {
            BrowserTarget::Library => if let Some(path) = paths.first() {
                library.set(path.clone());
                save_last_files();
            },
            BrowserTarget::Fasta => if let Some(path) = paths.first() {
                fasta.set(path.clone());
                save_last_files();
            },
            BrowserTarget::Config => if let Some(path) = paths.first() {
                config.set(path.clone());
                save_last_files();
            },
            BrowserTarget::ResultsDir => if let Some(path) = paths.first() { results_dir.set(path.clone()); },
            BrowserTarget::MzmlFiles => add_mzml_files(paths),
        }
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
        running.set(true);
        output.set("Starting job...".to_string());

        let run_config = RunConfig {
            library: library.read().clone(),
            fasta: fasta.read().clone(),
            config: {
                let c = config.read().clone();
                if c.is_empty() { None } else { Some(c) }
            },
            search_mode: *search_mode.read(),
            fdr_thresh: fdr_thresh.read().clone(),
            threads: threads.read().clone(),
            results_dir: {
                let r = results_dir.read().clone();
                if r.is_empty() { None } else { Some(r) }
            },
            mzml_files: mzml_files.read().clone(),
            img: None,
            check_image_updates: *check_image_updates.read(),
        };

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

    let params_panel = rsx! {
        // Left column - Parameters (1/3 width)
        form { class: "w-1/3 p-6 bg-white dark:bg-gray-800 rounded shadow flex flex-col gap-4 overflow-y-auto",
            onsubmit: on_submit,
            h2 { class: "text-xl font-bold mb-2 dark:text-gray-100", "Run Radiant+Fulcrum Workflow" }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Search Mode" }
                div { class: "flex gap-4 dark:text-gray-200",
                    label { class: "flex items-center gap-2",
                        input { r#type: "radio", name: "search_mode",
                            checked: *search_mode.read() == SearchMode::LibraryFree,
                            onchange: move |_| search_mode.set(SearchMode::LibraryFree) }
                        "Library-free"
                    }
                    label { class: "flex items-center gap-2",
                        input { r#type: "radio", name: "search_mode",
                            checked: *search_mode.read() == SearchMode::Mbr,
                            onchange: move |_| search_mode.set(SearchMode::Mbr) }
                        "Match Between Runs (MBR)"
                    }
                }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "mzML Files" }
                div { class: "mt-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100 text-sm max-h-32 overflow-y-auto",
                    if mzml_files.read().is_empty() {
                        "No files selected"
                    } else {
                        for (idx, file) in mzml_files.read().iter().enumerate() {
                            div { class: "py-0.5 flex items-center justify-between gap-2 hover:bg-gray-200 dark:hover:bg-gray-700 px-1 rounded group",
                                span { class: "flex-1 truncate", title: "{file}", "{get_filename(file)}" }
                                button { class: "text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 opacity-0 group-hover:opacity-100 px-2 text-lg font-bold",
                                    r#type: "button",
                                    onclick: move |_| {
                                        let mut files = mzml_files.read().clone();
                                        files.remove(idx);
                                        mzml_files.set(files);
                                    },
                                    "×"
                                }
                            }
                        }
                    }
                }
                button { class: "mt-1 px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                    r#type: "button", onclick: pick_mzml, "Browse" }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Library" }
                div { class: "flex gap-2",
                    div { class: "flex-1 relative group",
                        input { class: "w-full p-2 pr-8 border rounded dark:bg-gray-900 dark:text-gray-100",
                            r#type: "text",
                            placeholder: "Select library file...",
                            value: "{get_filename(&library.read())}",
                            title: "{library}",
                            oninput: move |e| library.set(e.value().clone()),
                            required: true }
                        if !library.read().is_empty() {
                            button { class: "absolute right-2 top-1/2 -translate-y-1/2 px-1 text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 text-lg font-bold opacity-0 group-hover:opacity-100",
                                r#type: "button", onclick: clear_library, title: "Clear", "×" }
                        }
                    }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_library, "Browse" }
                }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "FASTA" }
                div { class: "flex gap-2",
                    div { class: "flex-1 relative group",
                        input { class: "w-full p-2 pr-8 border rounded dark:bg-gray-900 dark:text-gray-100",
                            r#type: "text",
                            placeholder: "Select FASTA file...",
                            value: "{get_filename(&fasta.read())}",
                            title: "{fasta}",
                            oninput: move |e| fasta.set(e.value().clone()),
                            required: true }
                        if !fasta.read().is_empty() {
                            button { class: "absolute right-2 top-1/2 -translate-y-1/2 px-1 text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 text-lg font-bold opacity-0 group-hover:opacity-100",
                                r#type: "button", onclick: clear_fasta, title: "Clear", "×" }
                        }
                    }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_fasta, "Browse" }
                }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Results Directory (optional)" }
                div { class: "flex gap-2",
                    input { class: "flex-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                        r#type: "text",
                        placeholder: "Select output directory...",
                        value: "{get_filename(&results_dir.read())}",
                        title: "{results_dir}",
                        oninput: move |e| results_dir.set(e.value().clone()) }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_results_dir, "Browse" }
                }
            }

            div { class: "flex flex-col gap-2 mt-2",
                button { class: "text-left text-sm font-medium dark:text-gray-200 hover:text-blue-600 dark:hover:text-blue-400",
                    r#type: "button",
                    onclick: move |_| {
                        let current = *show_advanced.read();
                        show_advanced.set(!current);
                    },
                    if *show_advanced.read() { "▼ Advanced" } else { "▶ Advanced" }
                }

                if *show_advanced.read() {
                    div { class: "flex flex-col gap-4 pl-4 border-l-2 border-gray-300 dark:border-gray-600",
                        div { class: "flex flex-col gap-1",
                            label { class: "text-sm font-medium dark:text-gray-200", "FDR Threshold" }
                            input { class: "p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                                r#type: "number", step: "0.001", min: "0", max: "1", value: "{fdr_thresh}",
                                oninput: move |e| fdr_thresh.set(e.value().clone()) }
                        }

                        div { class: "flex flex-col gap-1",
                            label { class: "text-sm font-medium dark:text-gray-200", "Radiant Config (optional)" }
                            div { class: "flex gap-2",
                                div { class: "flex-1 relative group",
                                    input { class: "w-full p-2 pr-8 border rounded dark:bg-gray-900 dark:text-gray-100",
                                        r#type: "text", placeholder: "Select .radiantConfig file...",
                                        value: "{get_filename(&config.read())}",
                                        title: "{config}",
                                        oninput: move |e| config.set(e.value().clone()) }
                                    if !config.read().is_empty() {
                                        button { class: "absolute right-2 top-1/2 -translate-y-1/2 px-1 text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 text-lg font-bold opacity-0 group-hover:opacity-100",
                                            r#type: "button", onclick: clear_config, title: "Clear", "×" }
                                    }
                                }
                                button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                                    r#type: "button", onclick: pick_config, "Browse" }
                            }
                        }

                        div { class: "flex flex-col gap-1",
                            label { class: "text-sm font-medium dark:text-gray-200", "Threads (0 = auto)" }
                            input { class: "p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                                r#type: "number", min: "0", value: "{threads}",
                                oninput: move |e| threads.set(e.value().clone()) }
                        }

                        div { class: "flex flex-col gap-1",
                            label { class: "flex items-center gap-2 text-sm font-medium dark:text-gray-200 cursor-pointer",
                                input { r#type: "checkbox",
                                    checked: *check_image_updates.read(),
                                    onchange: move |e| check_image_updates.set(e.checked()) }
                                "Check for Docker image updates"
                            }
                        }
                    }
                }
            }

            button { class: "mt-2 py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded disabled:opacity-50",
                r#type: "submit", disabled: *running.read(), "Run" }
        }
    };

    let output_text = output.read().clone();
    let output_panel = output_panel(output_text);
    cli_form_layout(file_browser_element, params_panel, output_panel)
}
