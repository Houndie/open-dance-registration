use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::pages::Routes;

#[component]
pub fn Breadcrumb(cx: Scope, items: Vec<(String, Option<Routes>)>) -> Element {
    let nav = use_navigator(cx);

    cx.render(rsx! {
        nav {
            class: "breadcrumb",
            ul {
                items.iter().cloned().map(|(label, route)| {
                    rsx!{
                        li {
                            a {
                                prevent_default: "onclick",
                                onclick: move |_| {
                                    if let Some(route) = route.as_ref() {
                                        nav.push(route.clone());
                                    }
                                },
                                "{label}"
                            }
                        }
                    }
                })
            }
        }
    })
}
