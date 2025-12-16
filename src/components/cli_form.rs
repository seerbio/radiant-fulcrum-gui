use dioxus::prelude::*;
use std::sync::mpsc;
use crate::runner::{SearchMode, RunConfig, run_pythia_scry};

#[component]
pub fn CliForm() -> Element {
    let mut library = use_signal(String::new);
    let mut fasta = use_signal(String::new);
    let mut config = use_signal(String::new);
    let mut search_mode = use_signal(|| SearchMode::LibraryFree);
    let mut fdr_thresh = use_signal(|| "0.01".to_string());
    let mut threads = use_signal(|| "0".to_string());
    let mut results_dir = use_signal(String::new);
    let mzml_files = use_signal(Vec::<String>::new);
    let mut output = use_signal(String::new);
    let mut running = use_signal(|| false);

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

    let pick_results_dir = move |_| {
        let results_dir = results_dir.clone();
        spawn(async move {
            if let Some(path) = rfd::AsyncFileDialog::new().pick_folder().await {
                results_dir.clone().set(path.path().display().to_string());
            }
        });
    };

    let on_submit = move |_| {
        if *running.read() { return; }
        running.set(true);
        output.set(String::new());

        let run_config = RunConfig {
            library: library.read().clone(),
            fasta: fasta.read().clone(),
            config: Some(config.read().clone()),
            search_mode: *search_mode.read(),
            fdr_thresh: fdr_thresh.read().clone(),
            threads: threads.read().clone(),
            results_dir: Some(results_dir.read().clone()),
            mzml_files: mzml_files.read().clone(),
            img: None,
        };

        let (tx, rx) = mpsc::channel::<String>();

        std::thread::spawn(move || {
            let result = run_pythia_scry(run_config, |line| {
                let _ = tx.send(line.to_string());
            });
            match result {
                Ok(code) => { let _ = tx.send(format!("\n--- Process exited with code {} ---", code)); }
                Err(e) => { let _ = tx.send(format!("\n--- Failed to run: {} ---", e)); }
            }
        });

        let output = output.clone();
        let running = running.clone();
        spawn(async move {
            let mut output = output;
            let mut running = running;
            loop {
                match rx.try_recv() {
                    Ok(line) => {
                        let mut current = output.read().clone();
                        if !current.is_empty() { current.push('\n'); }
                        current.push_str(&line);
                        output.set(current);
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // Yield to allow UI updates, then continue polling
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
            running.set(false);
        });
    };

    let mzml_display = mzml_files.read().join(", ");

    rsx! {
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
                r#type: "submit", disabled: "{running}", "Run" }
        }
        div { class: "max-w-xl mx-auto mt-4 p-4 bg-gray-100 dark:bg-gray-900 rounded shadow text-xs dark:text-gray-100 whitespace-pre-wrap overflow-auto h-64",
            "{output}" }
    }
}
