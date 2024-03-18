use dioxus::prelude::*;
use dioxus_router::prelude::*;

use super::Menu as GenericMenu;
use crate::pages::Routes;

#[derive(Clone, PartialEq)]
pub enum MenuItem {
    None,
    Home,
}

impl MenuItem {
    fn is_active(&self, this: &MenuItem) -> &'static str {
        if *self == *this {
            "is-active"
        } else {
            ""
        }
    }
}

#[component]
pub fn Menu(cx: Scope, highlight: Option<MenuItem>) -> Element {
    let nav = use_navigator(cx);
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    cx.render(rsx! {
        GenericMenu {
            title: "ODR Admin",
            p {
                class: "menu-label",
                "General"
            }
            ul {
                class: "menu-list",
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::Home),
                        onclick: |_| { nav.push(Routes::OrganizationsPage); },
                        "Home"
                    }
                }
            }
        }
    })
}
