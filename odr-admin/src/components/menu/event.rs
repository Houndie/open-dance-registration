use dioxus::prelude::*;
use dioxus_router::prelude::*;

use super::Menu as GenericMenu;
use crate::pages::Routes;

#[derive(Clone, PartialEq)]
pub enum MenuItem {
    None,
    EventHome,
    RegistrationSchema,
    Registrations,
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
    cx: Scope,
    event_name: String,
    event_id: String,
    highlight: Option<MenuItem>,
) -> Element {
    let nav = use_navigator(cx);
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    cx.render(rsx! {
        GenericMenu {
            title: "{event_name}",
            p {
                class: "menu-label",
                "General"
            }
            ul {
                class: "menu-list",
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::EventHome),
                        onclick: |_| { nav.push(Routes::EventPage { id: event_id.clone() }); },
                        "Event Home"
                    }
                }
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::Registrations),
                        onclick: |_| { nav.push(Routes::RegistrationPage { event_id: event_id.clone() }); },
                        "Registrations"
                    }
                }
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::RegistrationSchema),
                        onclick: |_| { nav.push(Routes::RegistrationSchemaPage { id: event_id.clone() }); },
                        "Registration Schema"
                    }
                }
            }
        }
    })
}
