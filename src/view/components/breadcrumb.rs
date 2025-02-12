use dioxus::prelude::*;

use crate::view::app::Routes;

#[component]
pub fn Breadcrumb(items: ReadOnlySignal<Vec<(String, Option<Routes>)>>) -> Element {
    let nav = use_navigator();

    rsx! {
        nav {
            class: "breadcrumb",
            ul {
                { items.read().iter().cloned().map(|(label, route)| {
                    rsx!{
                        li {
                            a {
                                onclick: move |e| {
                                    e.prevent_default();
                                    if let Some(route) = route.as_ref() {
                                        nav.push(route.clone());
                                    }
                                },
                                "{label}"
                            }
                        }
                    }
                }) }
            }
        }
    }
}
