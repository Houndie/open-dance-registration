use common::proto::{
    self, event_query, organization_query, string_query, EventQuery, Organization,
    OrganizationQuery, QueryEventsRequest, QueryOrganizationsRequest, StringQuery,
};
use dioxus::prelude::*;

use crate::{
    server_functions::{
        event::query as query_events, organization::query as query_organizations, ProtoWrapper,
    },
    view::{
        app::Routes,
        components::{
            menu::Menu as GenericMenu, page::Page as GenericPage, with_toasts::WithToasts,
        },
    },
};

#[component]
pub fn Page(id: ReadOnlySignal<String>) -> Element {
    let nav = use_navigator();
    let events_response = use_server_future(move || {
        query_events(ProtoWrapper(QueryEventsRequest {
            query: Some(EventQuery {
                query: Some(event_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(id.read().clone())),
                })),
            }),
        }))
    })?;

    let ProtoWrapper(mut events_response) = match events_response() {
        Some(Ok(res)) => res,
        Some(Err(e)) => {
            return rsx! {
                WithToasts{
                    initial_errors: vec![e.to_string()],
                }
            };
        }
        None => return rsx! {},
    };

    if events_response.events.is_empty() {
        nav.push(Routes::NotFound);
        return rsx! {};
    }

    let event = events_response.events.remove(0);
    let organization_id = event.organization_id.clone();

    let organizations_response = use_server_future(move || {
        query_organizations(ProtoWrapper(QueryOrganizationsRequest {
            query: Some(OrganizationQuery {
                query: Some(organization_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(organization_id.clone())),
                })),
            }),
        }))
    })?;

    let ProtoWrapper(mut organizations_response) = match organizations_response() {
        Some(Ok(res)) => res,
        Some(Err(e)) => {
            return rsx! {
                WithToasts{
                    initial_errors: vec![e.to_string()],
                }
            };
        }
        None => return rsx! {},
    };

    let organization = organizations_response.organizations.remove(0);

    rsx! {
        WithToasts{
            ServerRenderedPage {
                org: organization,
                event: event,
            }
        }
    }
}

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
                        class: highlight.is_active(&MenuItem::EventHome),
                        onclick: move |e| {
                            e.prevent_default();
                            nav.push(Routes::EventPage { id: event_id.read().clone() });
                        },
                        "Event Home"
                    }
                }
                li {
                    a {
                        prevent_default: "onclick",
                        class: highlight.is_active(&MenuItem::Registrations),
                        onclick: move |_| { /*nav.push(Routes::RegistrationPage { event_id: event_id.read().clone() }); */},
                        "Registrations"
                    }
                }
                li {
                    a {
                        class: highlight.is_active(&MenuItem::RegistrationSchema),
                        onclick: move |e| {
                            e.prevent_default();
                            nav.push(Routes::RegistrationSchemaPage { id: event_id.read().clone() });
                        },
                        "Registration Schema"
                    }
                }
            }
        }
    }
}

#[component]
fn ServerRenderedPage(org: Organization, event: proto::Event) -> Element {
    let menu = rsx! {
        Menu {
            event_name: event.name.clone(),
            event_id: event.id,
            highlight: MenuItem::EventHome,
        }
    };

    rsx! {
        GenericPage {
            title: "Event Home".to_string(),
            breadcrumb: vec![
                ("Home".to_owned(), Some(Routes::LandingPage)),
                (org.name.clone(), Some(Routes::OrganizationPage { org_id: org.id.clone() })),
                (event.name.clone(), None),
            ],
            menu: menu,
            div {
            }
        }
    }
}
