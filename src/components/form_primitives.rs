use dioxus::prelude::*;
use super::button::{Button as DxButton, ButtonVariant};
use super::input::Input as DxInput;
use super::label::Label as DxLabel;

#[component]
pub fn FormSection(title: String, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-col gap-1",
            DxLabel { class: "text-sm font-medium dark:text-gray-200", html_for: "", "{title}" }
            {children}
        }
    }
}

#[component]
pub fn FieldRow(children: Element) -> Element {
    rsx! {
        div { class: "flex gap-2",
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
        DxInput {
            class: class,
            r#type: "text",
            placeholder: "{placeholder}",
            value: "{value}",
            title: "{title}",
            required: required,
            oninput: move |e: Event<FormData>| oninput.call(e.value().clone())
        }
    }
}

#[component]
pub fn BrowseButton(onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        DxButton {
            variant: ButtonVariant::Secondary,
            class: "px-3 py-2 bg-gray-200 dark:bg-gray-700 rounded hover:bg-gray-300 dark:hover:bg-gray-600 dark:text-gray-100",
            r#type: "button",
            onclick: move |e| onclick.call(e),
            "Browse"
        }
    }
}

#[component]
pub fn ClearButton(onclick: EventHandler<MouseEvent>, #[props(default = "Clear".to_string())] title: String) -> Element {
    rsx! {
        DxButton {
            variant: ButtonVariant::Ghost,
            class: "absolute right-2 top-1/2 -translate-y-1/2 px-1 text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 text-lg font-bold opacity-0 group-hover:opacity-100",
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
                        class: "w-full p-2 pr-8 border rounded dark:bg-gray-900 dark:text-gray-100".to_string(),
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
