use dioxus::prelude::*;
use dioxus_primitives::radio_group::{self, RadioGroupProps, RadioItemProps};
use crate::components::class_utils::with_base_class;

#[component]
pub fn RadioGroup(props: RadioGroupProps) -> Element {
    let attributes = with_base_class(props.attributes, "radio-group");

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("./style.css") }
        radio_group::RadioGroup {
            value: props.value,
            default_value: props.default_value,
            on_value_change: props.on_value_change,
            disabled: props.disabled,
            required: props.required,
            name: props.name,
            horizontal: props.horizontal,
            roving_loop: props.roving_loop,
            attributes: attributes,
            {props.children}
        }
    }
}

#[component]
pub fn RadioItem(props: RadioItemProps) -> Element {
    rsx! {
        radio_group::RadioItem {
            class: "radio-item",
            value: props.value,
            index: props.index,
            disabled: props.disabled,
            attributes: props.attributes,
            {props.children}
        }
    }
}
