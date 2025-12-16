use dioxus::prelude::*;
use crate::server_fns::{SearchMode, RunConfig, start_pythia_scry, get_job_status};

#[cfg(not(feature = "desktop"))]
use crate::components::{FileBrowser, FileBrowserMode};

/// Platform-agnostic sleep function
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

#[derive(Clone, Copy, PartialEq)]
enum BrowserTarget {
    Library,
    Fasta,
    Config,
    ResultsDir,
    MzmlFiles,
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

    // File browser state (for non-desktop builds)
    #[allow(unused_variables)]
    let mut show_browser = use_signal(|| None::<BrowserTarget>);

    // Desktop-only: use native file dialogs
    #[cfg(feature = "desktop")]
    let pick_library = move |_| {
        let library = library.clone();
        spawn(async move {
            if let Some(path) = rfd::AsyncFileDialog::new()
                .add_filter("Library", &["tsv", "parquet", "speclib"])
                .pick_file().await
            {
                library.clone().set(path.path().display().to_string());
            }
        });
    };

    #[cfg(feature = "desktop")]
    let pick_fasta = move |_| {
        let fasta = fasta.clone();
        spawn(async move {
            if let Some(path) = rfd::AsyncFileDialog::new()
                .add_filter("FASTA", &["fasta", "fas"])
                .pick_file().await
            {
                fasta.clone().set(path.path().display().to_string());
            }
        });
    };

    #[cfg(feature = "desktop")]
    let pick_config = move |_| {
        let config = config.clone();
        spawn(async move {
            if let Some(path) = rfd::AsyncFileDialog::new()
                .add_filter("Pythia Config", &["pythiaConfig"])
                .pick_file().await
            {
                config.clone().set(path.path().display().to_string());
            }
        });
    };

    #[cfg(feature = "desktop")]
    let pick_mzml = move |_| {
        let mzml_files = mzml_files.clone();
        spawn(async move {
            let files = rfd::AsyncFileDialog::new()
                .add_filter("mzML", &["mzML", "mzml"])
                .pick_files().await;
            if let Some(files) = files {
                let paths: Vec<String> = files.iter().map(|f| f.path().display().to_string()).collect();
                mzml_files.clone().set(paths);
            }
        });
    };

    #[cfg(feature = "desktop")]
    let pick_results_dir = move |_| {
        let results_dir = results_dir.clone();
        spawn(async move {
            if let Some(path) = rfd::AsyncFileDialog::new().pick_folder().await {
                results_dir.clone().set(path.path().display().to_string());
            }
        });
    };

    // Web/Server: use server-side file browser
    #[cfg(not(feature = "desktop"))]
    let pick_library = move |_| {
        show_browser.set(Some(BrowserTarget::Library));
    };

    #[cfg(not(feature = "desktop"))]
    let pick_fasta = move |_| {
        show_browser.set(Some(BrowserTarget::Fasta));
    };

    #[cfg(not(feature = "desktop"))]
    let pick_config = move |_| {
        show_browser.set(Some(BrowserTarget::Config));
    };

    #[cfg(not(feature = "desktop"))]
    let pick_mzml = move |_| {
        show_browser.set(Some(BrowserTarget::MzmlFiles));
    };

    #[cfg(not(feature = "desktop"))]
    let pick_results_dir = move |_| {
        show_browser.set(Some(BrowserTarget::ResultsDir));
    };

    // Handle file browser selection (web/server only)
    #[cfg(not(feature = "desktop"))]
    let on_browser_select = move |paths: Vec<String>| {
        if let Some(target) = *show_browser.read() {
            match target {
                BrowserTarget::Library => {
                    if let Some(path) = paths.first() {
                        library.set(path.clone());
                    }
                }
                BrowserTarget::Fasta => {
                    if let Some(path) = paths.first() {
                        fasta.set(path.clone());
                    }
                }
                BrowserTarget::Config => {
                    if let Some(path) = paths.first() {
                        config.set(path.clone());
                    }
                }
                BrowserTarget::ResultsDir => {
                    if let Some(path) = paths.first() {
                        results_dir.set(path.clone());
                    }
                }
                BrowserTarget::MzmlFiles => {
                    mzml_files.set(paths);
                }
            }
        }
        show_browser.set(None);
    };

    #[cfg(not(feature = "desktop"))]
    let on_browser_cancel = move |_| {
        show_browser.set(None);
    };

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
        };

        spawn(async move {
            match start_pythia_scry(run_config).await {
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

    let mzml_display = mzml_files.read().join(", ");

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
                extensions: vec!["pythiaConfig".into()],
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

    rsx! {
        // File browser modal (web/server only)
        {file_browser_element}

        form { class: "max-w-xl mx-auto p-6 bg-white dark:bg-gray-800 rounded shadow flex flex-col gap-4",
            onsubmit: on_submit,
            h2 { class: "text-xl font-bold mb-2 dark:text-gray-100", "Run Pythia Scry Workflow" }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Library" }
                div { class: "flex gap-2",
                    input { class: "flex-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                        r#type: "text", placeholder: "Select library file...", value: "{library}",
                        oninput: move |e| library.set(e.value().clone()), required: true }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_library, "Browse" }
                }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "FASTA" }
                div { class: "flex gap-2",
                    input { class: "flex-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                        r#type: "text", placeholder: "Select FASTA file...", value: "{fasta}",
                        oninput: move |e| fasta.set(e.value().clone()), required: true }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_fasta, "Browse" }
                }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Config (optional)" }
                div { class: "flex gap-2",
                    input { class: "flex-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                        r#type: "text", placeholder: "Select .pythiaConfig file...", value: "{config}",
                        oninput: move |e| config.set(e.value().clone()) }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_config, "Browse" }
                }
            }

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
                label { class: "text-sm font-medium dark:text-gray-200", "FDR Threshold" }
                input { class: "p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                    r#type: "number", step: "0.001", min: "0", max: "1", value: "{fdr_thresh}",
                    oninput: move |e| fdr_thresh.set(e.value().clone()) }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Threads (0 = auto)" }
                input { class: "p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                    r#type: "number", min: "0", value: "{threads}",
                    oninput: move |e| threads.set(e.value().clone()) }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "Results Directory (optional)" }
                div { class: "flex gap-2",
                    input { class: "flex-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                        r#type: "text", placeholder: "Select output directory...", value: "{results_dir}",
                        oninput: move |e| results_dir.set(e.value().clone()) }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_results_dir, "Browse" }
                }
            }

            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium dark:text-gray-200", "mzML Files" }
                div { class: "flex gap-2",
                    input { class: "flex-1 p-2 border rounded dark:bg-gray-900 dark:text-gray-100",
                        r#type: "text", placeholder: "Select mzML files...", value: "{mzml_display}",
                        readonly: true }
                    button { class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
                        r#type: "button", onclick: pick_mzml, "Browse" }
                }
            }

            button { class: "mt-2 py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded disabled:opacity-50",
                r#type: "submit", disabled: *running.read(), "Run" }
        }
        div { class: "max-w-xl mx-auto mt-4 p-4 bg-gray-100 dark:bg-gray-900 rounded shadow text-xs dark:text-gray-100 whitespace-pre-wrap overflow-auto h-64",
            "{output}" }
    }
}
