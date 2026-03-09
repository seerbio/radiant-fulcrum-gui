use dioxus::prelude::*;
use super::button::{Button as DxButton, ButtonVariant};

const FORM_SECTION_CLASS: &str = "flex flex-col gap-1";
const FIELD_LABEL_CLASS: &str = "text-sm font-medium dark:text-gray-200";
const FIELD_ROW_CLASS: &str = "flex gap-2";
const BROWSE_BUTTON_CLASS: &str =
    "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100";
const CLEAR_BUTTON_CLASS: &str =
    "absolute right-2 top-1/2 -translate-y-1/2 px-1 text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 text-lg font-bold opacity-0 group-hover:opacity-100";
const PATH_INPUT_CLASS: &str = "w-full p-2 pr-8 border rounded dark:bg-gray-900 dark:text-gray-100";

#[component]
pub fn FormSection(title: String, children: Element) -> Element {
    rsx! {
        div { class: FORM_SECTION_CLASS,
            label { class: FIELD_LABEL_CLASS, "{title}" }
            {children}
        }
    }
}

#[component]
pub fn FieldRow(children: Element) -> Element {
    rsx! {
        div { class: FIELD_ROW_CLASS,
            {children}
        }
    }
}

#[component]
pub fn PathInput(
    value: String,
    title: String,
    placeholder: String,
    oninput: EventHandler<String>,
    #[props(default = false)] required: bool,
    #[props(default = "w-full p-2 border rounded dark:bg-gray-900 dark:text-gray-100".to_string())]
    class: String,
) -> Element {
    rsx! {
        input {
            class: class,
            r#type: "text",
            placeholder: "{placeholder}",
            value: "{value}",
            title: "{title}",
            required: required,
            oninput: move |e: Event<FormData>| oninput.call(e.value().clone()),
        }
    }
}

#[component]
pub fn BrowseButton(onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        DxButton {
            variant: ButtonVariant::Secondary,
            class: BROWSE_BUTTON_CLASS,
            r#type: "button",
            onclick: move |e| onclick.call(e),
            "Browse"
        }
    }
}

#[component]
pub fn ClearButton(onclick: EventHandler<MouseEvent>, #[props(default = "Clear".to_string())] title: String) -> Element {
    rsx! {
        button {
            class: CLEAR_BUTTON_CLASS,
            r#type: "button",
            onclick: move |e| onclick.call(e),
            title: "{title}",
            "×"
        }
    }
}

#[component]
pub fn FilePathField(
    title: String,
    placeholder: String,
    value: String,
    full_path: String,
    oninput: EventHandler<String>,
    onbrowse: EventHandler<MouseEvent>,
    onclear: EventHandler<MouseEvent>,
    #[props(default = false)] required: bool,
) -> Element {
    rsx! {
        FormSection { title: title,
            FieldRow {
                div { class: "flex-1 relative group",
                    PathInput {
                        class: PATH_INPUT_CLASS.to_string(),
                        placeholder: placeholder,
                        value: value,
                        title: full_path.clone(),
                        required: required,
                        oninput: move |next| oninput.call(next),
                    }
                    if !full_path.is_empty() {
                        ClearButton { onclick: move |e| onclear.call(e), title: "Clear".to_string() }
                    }
                }
                BrowseButton { onclick: onbrowse }
            }
        }
    }
}
