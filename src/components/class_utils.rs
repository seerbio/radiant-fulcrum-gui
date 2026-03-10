use dioxus::prelude::*;
use dioxus_core::AttributeValue;

pub(crate) fn with_base_class(
    mut attributes: Vec<Attribute>,
    base_class: &'static str,
) -> Vec<Attribute> {
    let mut has_class = false;

    for attr in &mut attributes {
        if attr.name == "class" && attr.namespace.is_none() {
            has_class = true;
            if let AttributeValue::Text(value) = &mut attr.value {
                if value.is_empty() {
                    *value = base_class.to_string();
                } else if !value.split_whitespace().any(|class| class == base_class) {
                    value.push(' ');
                    value.push_str(base_class);
                }
            }
        }
    }

    if !has_class {
        attributes.push(Attribute::new("class", base_class, None, false));
    }

    attributes
}
