use dioxus::prelude::*;
use dioxus_router::prelude::*;

use super::Menu as GenericMenu;
use crate::pages::Routes;

#[derive(Clone, PartialEq)]
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
pub fn Menu(cx: Scope, org_name: String, org_id: String, highlight: Option<MenuItem>) -> Element {
    let nav = use_navigator(cx);
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    cx.render(rsx! {
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
                        onclick: |_| { nav.push(Routes::EventsPage { org_id: org_id.clone() }); },
                        "Organization Home"
                    }
                }
            }
        }
    })
}
