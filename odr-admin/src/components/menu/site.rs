use dioxus::prelude::*;

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
pub fn Menu(highlight: Option<MenuItem>) -> Element {
    let nav = use_navigator();
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    rsx! {
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
                        onclick: move |_| { nav.push(Routes::OrganizationsPage); },
                        "Home"
                    }
                }
            }
        }
    }
}
