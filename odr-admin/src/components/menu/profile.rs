use dioxus::prelude::*;
use dioxus_router::prelude::*;

use super::Menu as GenericMenu;
use crate::pages::Routes;

#[derive(Clone, PartialEq)]
pub enum MenuItem {
    None,
    AccountSettings,
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
pub fn Menu(cx: Scope, user_name: String, highlight: Option<MenuItem>) -> Element {
    let nav = use_navigator(cx);
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    cx.render(rsx! {
        GenericMenu {
            title: "{user_name}",
            p {
                class: "menu-label",
                "Account",
            }
            ul {
                class: "menu-list",
                li {
                    prevent_default: "onclick",
                    class: highlight.is_active(&MenuItem::AccountSettings),
                    a {
                        onclick: |_| { nav.push(Routes::ProfilePage); },
                        "Account Settings",
                    }
                }
            }

        }
    })
}
