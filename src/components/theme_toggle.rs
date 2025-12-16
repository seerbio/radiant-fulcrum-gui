use dioxus::prelude::*;

#[component]
pub fn ThemeToggle() -> Element {
    let mut is_dark = use_signal(|| true);
    let mut initialized = use_signal(|| false);

    use_effect(move || {
        if !initialized() {
            spawn(async move {
                // Use dark_light crate to detect system theme
                match dark_light::detect() {
                    Ok(dark_light::Mode::Dark) => {
                        is_dark.set(true);
                    }
                    Ok(dark_light::Mode::Light) => {
                        is_dark.set(false);
                    }
                    _ => { }
                }
                initialized.set(true);
            });
        }
    });

    let toggle_theme = move |_| {
        is_dark.set(!is_dark());
    };

    use_effect(move || {
        if !initialized() {
            return;
        }
        let dark = is_dark();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    if let Some(html) = document.document_element() {
                        let class_list = html.class_list();
                        if dark {
                            let _ = class_list.add_1("dark");
                        } else {
                            let _ = class_list.remove_1("dark");
                        }
                    }
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // For desktop, we'll use document::eval to manipulate the DOM
            let js = if dark {
                "document.documentElement.classList.add('dark')"
            } else {
                "document.documentElement.classList.remove('dark')"
            };
            spawn(async move {
                let _ = document::eval(js).await;
            });
        }
    });

    rsx! {
        button {
            class: "fixed top-4 right-4 p-2 rounded-lg bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors duration-200 z-50",
            onclick: toggle_theme,
            title: if is_dark() { "Switch to light mode" } else { "Switch to dark mode" },
            if is_dark() {
                // Sun icon (show when in dark mode to switch to light)
                svg {
                    class: "w-6 h-6 text-white",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    view_box: "0 0 24 24",
                    // Sun rays
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        d: "M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
                    }
                }
            } else {
                // Moon icon (show when in light mode to switch to dark)
                svg {
                    class: "w-6 h-6 text-gray-700",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    view_box: "0 0 24 24",
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        d: "M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
                    }
                }
            }
        }
    }
}
