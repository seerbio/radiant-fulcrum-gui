use dioxus::prelude::*;

#[component]
pub fn ScrollArea(
    #[props(default = "overflow-auto".to_string())] class: String,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: class,
            ..attributes,
            {children}
        }
    }
}
