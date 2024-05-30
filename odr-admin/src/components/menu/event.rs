use dioxus::prelude::*;

use super::Menu as GenericMenu;
use crate::pages::Routes;

#[derive(Clone, Copy, PartialEq)]
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
    event_name: ReadOnlySignal<String>,
    event_id: ReadOnlySignal<String>,
    highlight: Option<MenuItem>,
) -> Element {
    let nav = use_navigator();
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    rsx! {
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
                        onclick: move |_| { nav.push(Routes::EventPage { id: event_id.read().clone() }); },
                        "Event Home"
                    }
                }
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::Registrations),
                        onclick: move |_| { nav.push(Routes::RegistrationPage { event_id: event_id.read().clone() }); },
                        "Registrations"
                    }
                }
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::RegistrationSchema),
                        onclick: move |_| { nav.push(Routes::RegistrationSchemaPage { id: event_id.read().clone() }); },
                        "Registration Schema"
                    }
                }
            }
        }
    }
}
