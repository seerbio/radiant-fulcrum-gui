use dioxus::prelude::*;
use crate::server_fns::{DirectoryListing, FileEntry, list_directory};
use crate::components::cli_form::{LAST_DIRECTORY, update_last_dir};
use crate::components::dialog::{DialogContent, DialogRoot};

const BROWSER_DIALOG_CLASS: &str =
    "w-full max-w-2xl max-h-[80vh] flex flex-col bg-white dark:bg-gray-800 rounded-lg shadow-xl";
const HEADER_CLASS: &str = "p-4 border-b dark:border-gray-700 flex justify-between items-center";
const TITLE_CLASS: &str = "text-lg font-semibold dark:text-gray-100";
const CLOSE_BUTTON_CLASS: &str = "text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200";
const PATH_BAR_CLASS: &str =
    "px-4 py-2 bg-gray-100 dark:bg-gray-900 text-sm dark:text-gray-300 flex items-center gap-2";
const UP_BUTTON_CLASS: &str = "px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600";
const FILE_LIST_CLASS: &str = "flex-1 overflow-auto p-2";
const LOADING_CLASS: &str = "flex items-center justify-center h-32 dark:text-gray-300";
const ERROR_CLASS: &str = "text-red-500 p-4";
const ENTRIES_CLASS: &str = "space-y-1";
const FOOTER_CLASS: &str = "p-4 border-t dark:border-gray-700 flex justify-between items-center gap-2";
const FOOTER_META_CLASS: &str = "text-sm dark:text-gray-400";
const FOOTER_ACTIONS_CLASS: &str = "flex gap-2";
const SELECT_CURRENT_BUTTON_CLASS: &str = "px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700";
const CANCEL_BUTTON_CLASS: &str =
    "px-4 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-200";
const SELECT_BUTTON_CLASS: &str = "px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50";

#[derive(Clone, PartialEq)]
pub enum FileBrowserMode {
    File { extensions: Vec<String> },
    Directory,
    MultiFile { extensions: Vec<String> },
}

#[component]
pub fn FileBrowser(
    mode: FileBrowserMode,
    on_select: EventHandler<Vec<String>>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut listing = use_signal(|| None::<DirectoryListing>);
    let mut selected = use_signal(Vec::<String>::new);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);

    // Load initial directory
    use_effect(move || {
        spawn(async move {
            loading.set(true);
            let start_path = LAST_DIRECTORY.read().clone();
            match list_directory(start_path).await {
                Ok(dir) => {
                    listing.set(Some(dir));
                    error.set(None);
                }
                Err(e) => {
                    error.set(Some(format!("{}", e)));
                }
            }
            loading.set(false);
        });
    });

    let navigate_to = move |path: String| {
        spawn(async move {
            loading.set(true);
            match list_directory(Some(path.clone())).await {
                Ok(dir) => {
                    listing.set(Some(dir));
                    error.set(None);
                    selected.set(Vec::new());
                    update_last_dir(&path);
                }
                Err(e) => {
                    error.set(Some(format!("{}", e)));
                }
            }
            loading.set(false);
        });
    };

    let mode_for_filter = mode.clone();
    let filter_entry = move |entry: &FileEntry| -> bool {
        if entry.is_dir {
            return true; // Always show directories for navigation
        }
        match &mode_for_filter {
            FileBrowserMode::Directory => false, // Don't show files in directory mode
            FileBrowserMode::File { extensions } | FileBrowserMode::MultiFile { extensions } => {
                if extensions.is_empty() {
                    return true;
                }
                let name_lower = entry.name.to_lowercase();
                extensions.iter().any(|ext| name_lower.ends_with(&format!(".{}", ext.to_lowercase())))
            }
        }
    };

    let on_select_clone = on_select.clone();
    let mode_for_confirm = mode.clone();
    let confirm_selection = move |_| {
        let sel = selected.read();
        if !sel.is_empty() {
            if let Some(ref dir) = *listing.read() {
                update_last_dir(&dir.current_path);
            }
            on_select_clone.call(sel.clone());
        } else if matches!(mode_for_confirm, FileBrowserMode::Directory) {
            if let Some(ref dir) = *listing.read() {
                update_last_dir(&dir.current_path);
                on_select_clone.call(vec![dir.current_path.clone()]);
            }
        }
    };

    let mode_for_current = mode.clone();
    let select_current_dir = move |_| {
        if let Some(ref dir) = *listing.read() {
            update_last_dir(&dir.current_path);
            on_select.call(vec![dir.current_path.clone()]);
        }
    };

    let mode_for_toggle = mode.clone();

    rsx! {
        DialogRoot {
            open: Some(true),
            on_open_change: move |is_open: bool| {
                if !is_open {
                    on_cancel.call(());
                }
            },
            DialogContent {
                class: BROWSER_DIALOG_CLASS.to_string(),
                // Header
                div { class: HEADER_CLASS,
                    h3 { class: TITLE_CLASS,
                        match &mode {
                            FileBrowserMode::File { .. } => "Select File",
                            FileBrowserMode::Directory => "Select Directory",
                            FileBrowserMode::MultiFile { .. } => "Select Files",
                        }
                    }
                    button {
                        class: CLOSE_BUTTON_CLASS,
                        onclick: move |_| on_cancel.call(()),
                        "✕"
                    }
                }

                // Current path
                if let Some(ref dir) = *listing.read() {
                    div { class: PATH_BAR_CLASS,
                        if let Some(ref parent) = dir.parent_path {
                            button {
                                class: UP_BUTTON_CLASS,
                                onclick: {
                                    let parent = parent.clone();
                                    move |_| navigate_to(parent.clone())
                                },
                                "↑ Up"
                            }
                        }
                        span { class: "truncate", "{dir.current_path}" }
                    }
                }

                // File list
                div { class: FILE_LIST_CLASS,
                    if *loading.read() {
                        div { class: LOADING_CLASS,
                            "Loading..."
                        }
                    } else if let Some(ref err) = *error.read() {
                        div { class: ERROR_CLASS,
                            "Error: {err}"
                        }
                    } else if let Some(ref dir) = *listing.read() {
                        div { class: ENTRIES_CLASS,
                            for entry in dir.entries.iter().filter(|e| filter_entry(e)) {
                                {
                                    let entry_path = entry.path.clone();
                                    let entry_name = entry.name.clone();
                                    let entry_is_dir = entry.is_dir;
                                    let is_selected = selected.read().contains(&entry.path);
                                    let mode_clone = mode_for_toggle.clone();
                                    rsx! {
                                        div {
                                            key: "{entry_path}",
                                            class: format!(
                                                "flex items-center gap-2 p-2 rounded cursor-pointer {} {}",
                                                if is_selected { "bg-blue-100 dark:bg-blue-900" } else { "hover:bg-gray-100 dark:hover:bg-gray-700" },
                                                "dark:text-gray-200"
                                            ),
                                            onclick: {
                                                let path = entry_path.clone();
                                                let is_dir = entry_is_dir;
                                                move |_| {
                                                    let mut sel = selected.write();
                                                    if let Some(pos) = sel.iter().position(|p| p == &path) {
                                                        sel.remove(pos);
                                                    } else {
                                                        match &mode_clone {
                                                            FileBrowserMode::MultiFile { .. } => {
                                                                if !is_dir {
                                                                    sel.push(path.clone());
                                                                }
                                                            }
                                                            FileBrowserMode::File { .. } => {
                                                                if !is_dir {
                                                                    sel.clear();
                                                                    sel.push(path.clone());
                                                                }
                                                            }
                                                            FileBrowserMode::Directory => {
                                                                if is_dir {
                                                                    sel.clear();
                                                                    sel.push(path.clone());
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                            ondoubleclick: {
                                                let path = entry_path.clone();
                                                move |_| {
                                                    if entry_is_dir {
                                                        navigate_to(path.clone());
                                                    }
                                                }
                                            },
                                            span { class: "text-lg",
                                                if entry_is_dir { "📁" } else { "📄" }
                                            }
                                            span { class: "truncate", "{entry_name}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Footer
                div { class: FOOTER_CLASS,
                    div { class: FOOTER_META_CLASS,
                        if !selected.read().is_empty() {
                            "{selected.read().len()} selected"
                        }
                    }
                    div { class: FOOTER_ACTIONS_CLASS,
                        if matches!(mode_for_current, FileBrowserMode::Directory) {
                            button {
                                class: SELECT_CURRENT_BUTTON_CLASS,
                                onclick: select_current_dir,
                                "Select Current"
                            }
                        }
                        button {
                            class: CANCEL_BUTTON_CLASS,
                            onclick: move |_| on_cancel.call(()),
                            "Cancel"
                        }
                        button {
                            class: SELECT_BUTTON_CLASS,
                            disabled: selected.read().is_empty(),
                            onclick: confirm_selection,
                            "Select"
                        }
                    }
                }
            }
        }
    }
}
