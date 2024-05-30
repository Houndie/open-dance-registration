use dioxus::prelude::*;

use super::Menu as GenericMenu;
use crate::pages::Routes;

#[derive(Clone, Copy, PartialEq)]
pub enum MenuItem {
    None,
    OrganizationHome,
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
pub fn Menu(
    org_name: ReadOnlySignal<String>,
    org_id: ReadOnlySignal<String>,
    highlight: Option<MenuItem>,
) -> Element {
    let nav = use_navigator();
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    rsx! {
        GenericMenu {
            title: "{org_name}",
            p {
                class: "menu-label",
                "General"
            }
            ul {
                class: "menu-list",
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::OrganizationHome),
                        onclick: move |_| { nav.push(Routes::EventsPage { org_id: org_id.read().clone() }); },
                        "Organization Home"
                    }
                }
            }
        }
    }
}
