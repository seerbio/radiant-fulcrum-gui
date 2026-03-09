#![cfg_attr(feature = "bundle", windows_subsystem = "windows")]

// The dioxus prelude contains a ton of common items used in dioxus apps. It's a good idea to import wherever you
// need dioxus
use dioxus::prelude::*;

use components::{CliForm, ThemeToggle};

mod components;
mod server_fns;
mod types;
mod storage;

#[cfg(not(feature = "web"))]
mod runner;

#[cfg(feature = "desktop")]
use dioxus::desktop::{Config, WindowBuilder};

// We can import assets in dioxus with the `asset!` macro. This macro takes a path to an asset relative to the crate root.
// The macro returns an `Asset` type that will display as the path to the asset in the browser or a local path in desktop bundles.
const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const THEME_TOKENS_CSS: Asset = asset!("/assets/theme-tokens.css");

fn main() {
    #[allow(unused_mut)]
    let mut builder = LaunchBuilder::new();

    #[cfg(feature = "desktop")]
    {
        let menu = None;

        let cfg = Config::new()
            .with_menu(menu)
            .with_window(
                WindowBuilder::new()
                    .with_title("Radiant")
                    .with_always_on_top(false)
            );
        builder = builder.with_cfg(cfg);
    }

    builder.launch(App);
}

/// App is the main component of our app. Components are the building blocks of dioxus apps. Each component is a function
/// that takes some props and returns an Element. In this case, App takes no props because it is the root of our app.
///
/// Components should be annotated with `#[component]` to support props, better error messages, and autocomplete
#[component]
fn App() -> Element {
    // The `rsx!` macro lets us define HTML inside of rust. It expands to an Element with all of our HTML inside.
    rsx! {
        // In addition to element and text (which we will see later), rsx can contain other components. In this case,
        // we are using the `document::Link` component to add a link to our favicon and main CSS file into the head of our app.
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        document::Link { rel: "stylesheet", href: THEME_TOKENS_CSS }

        div { class: "h-screen max-h-screen bg-gray-100 dark:bg-gray-900 overflow-hidden transition-colors duration-200",
            ThemeToggle {}
            CliForm {}
        }
    }
}
