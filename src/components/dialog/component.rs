use dioxus::prelude::*;
use dioxus_primitives::dialog::{
    self, DialogContentProps, DialogRootProps, DialogTitleProps,
};

#[component]
pub fn DialogRoot(props: DialogRootProps) -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: asset!("./style.css") }
        dialog::DialogRoot {
            id: props.id,
            is_modal: props.is_modal,
            open: props.open,
            default_open: props.default_open,
            on_open_change: props.on_open_change,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn DialogContent(props: DialogContentProps) -> Element {
    rsx! {
        dialog::DialogContent {
            id: props.id,
            class: props.class.or(Some("dialog-content".to_string())),
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn DialogTitle(props: DialogTitleProps) -> Element {
    rsx! {
        dialog::DialogTitle {
            id: props.id,
            attributes: props.attributes,
            {props.children}
        }
    }
}
