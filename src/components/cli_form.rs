use dioxus::prelude::*;
use std::process::Command;

#[component]
pub fn CliForm() -> Element {
    let mut library = use_signal(String::new);
    let mut fasta = use_signal(String::new);
    let mut config = use_signal(String::new);
    let mut mbr = use_signal(|| false);
    let mut fdr_thresh = use_signal(|| "0.01".to_string());
    let mut threads = use_signal(|| "0".to_string());
    let mut results_dir = use_signal(String::new);
    let mut mzml_files = use_signal(String::new);
    let mut output = use_signal(String::new);
    let mut running = use_signal(|| false);

    let on_submit = move |_| {
        if *running.read() { return; }
        running.set(true);
        output.set("Running...".to_string());
        let library = library.read().clone();
        let fasta = fasta.read().clone();
        let config = config.read().clone();
        let mbr = mbr.read().clone();
        let fdr_thresh = fdr_thresh.read().clone();
        let threads = threads.read().clone();
        let results_dir = results_dir.read().clone();
        let mzml_files = mzml_files.read().clone();
        let output = output.clone();
        let running = running.clone();
        spawn(async move {
            let mut output = output;
            let mut running = running;
            let mut args = vec![
                "--library", &library,
                "--fasta", &fasta,
                "--fdr-thresh", &fdr_thresh,
                "--threads", &threads,
            ];
            if !config.is_empty() { args.push("--config"); args.push(&config); }
            if mbr { args.push("--mbr"); }
            if !results_dir.is_empty() { args.push("--results-dir"); args.push(&results_dir); }
            for file in mzml_files.split_whitespace() { args.push(file); }
            let res = Command::new("docker")
                .arg("run")
                .arg("--rm")
                .arg("-v")
                .arg(format!("{}:/data", std::env::current_dir().unwrap().display()))
                .arg("pythia-scry-cli")
                .args(&args)
                .output();
            match res {
                Ok(out) => {
                    let mut s = String::new();
                    if !out.stdout.is_empty() { s.push_str(&String::from_utf8_lossy(&out.stdout)); }
                    if !out.stderr.is_empty() { s.push_str("\n---stderr---\n"); s.push_str(&String::from_utf8_lossy(&out.stderr)); }
                    output.set(s);
                }
                Err(e) => output.set(format!("Failed to run: {}", e)),
            }
            running.set(false);
        });
    };

    rsx! {
        form { class: "max-w-xl mx-auto p-6 bg-gray-800 rounded shadow flex flex-col gap-4",
            onsubmit: on_submit,
            h2 { class: "text-xl font-bold mb-2 text-gray-100", "Run Pythia Scry Workflow" }
            input { class: "p-2 border rounded bg-gray-900 text-gray-100",
                r#type: "text", placeholder: "Library URI", value: "{library}", oninput: move |e| library.set(e.value().clone()), required: true }
            input { class: "p-2 border rounded bg-gray-900 text-gray-100",
                r#type: "text", placeholder: "FASTA URI", value: "{fasta}", oninput: move |e| fasta.set(e.value().clone()), required: true }
            input { class: "p-2 border rounded bg-gray-900 text-gray-100",
                r#type: "text", placeholder: "Config URI (.pythiaConfig)", value: "{config}", oninput: move |e| config.set(e.value().clone()) }
            label { class: "flex items-center gap-2 text-gray-200",
                input { r#type: "checkbox", checked: "{mbr}", onchange: move |e| mbr.set(e.checked()) }
                "Match Between Runs (MBR)"
            }
            input { class: "p-2 border rounded bg-gray-900 text-gray-100",
                r#type: "number", step: "0.001", min: "0", placeholder: "FDR Threshold", value: "{fdr_thresh}", oninput: move |e| fdr_thresh.set(e.value().clone()) }
            input { class: "p-2 border rounded bg-gray-900 text-gray-100",
                r#type: "number", min: "0", placeholder: "Threads (0=auto)", value: "{threads}", oninput: move |e| threads.set(e.value().clone()) }
            input { class: "p-2 border rounded bg-gray-900 text-gray-100",
                r#type: "text", placeholder: "Results Directory", value: "{results_dir}", oninput: move |e| results_dir.set(e.value().clone()) }
            textarea { class: "p-2 border rounded h-20 bg-gray-900 text-gray-100",
                placeholder: "MZML files (space separated)", value: "{mzml_files}", oninput: move |e| mzml_files.set(e.value().clone()) }
            button { class: "mt-2 py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded disabled:opacity-50",
                r#type: "submit", disabled: "{running}", "Run" }
        }
        div { class: "max-w-xl mx-auto mt-4 p-4 bg-gray-800 rounded shadow text-xs text-gray-100 whitespace-pre-wrap overflow-auto h-64",
            "{output}" }
    }
}
