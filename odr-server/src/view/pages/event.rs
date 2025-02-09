use common::proto::{
    event_query, organization_query, string_query, EventQuery, OrganizationQuery,
    QueryEventsRequest, QueryOrganizationsRequest, StringQuery,
};
use dioxus::prelude::*;

use crate::{
    server_functions::{
        event::query as query_events, organization::query as query_organizations, ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{
            menu::Menu as GenericMenu, page::Page as GenericPage, with_toasts::WithToasts,
        },
    },
};

#[component]
pub fn Page(id: ReadOnlySignal<String>) -> Element {
    let nav = use_navigator();
    let results = use_server_future(move || async move {
        let mut events_response = query_events(QueryEventsRequest {
            query: Some(EventQuery {
                query: Some(event_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(id.read().clone())),
                })),
            }),
        })
        .await
        .map_err(Error::ServerFunctionError)?;

        let event = events_response.events.pop().ok_or(Error::NotFound)?;

        let mut organizations_response = query_organizations(QueryOrganizationsRequest {
            query: Some(OrganizationQuery {
                query: Some(organization_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(
                        event.organization_id.clone(),
                    )),
                })),
            }),
        })
        .await
        .map_err(Error::ServerFunctionError)?;

        let organization = organizations_response
            .organizations
            .pop()
            .ok_or(Error::Misc("organization not found".to_owned()))?;

        Ok((ProtoWrapper(organization), ProtoWrapper(event)))
    })?;

    let (organization, event) = match results() {
        None => return rsx! {},
        Some(Ok((ProtoWrapper(organization), ProtoWrapper(event)))) => (organization, event),
        Some(Err(Error::NotFound)) => {
            nav.push(Routes::NotFound);
            return rsx! {};
        }
        Some(Err(e)) => {
            return rsx! {
                WithToasts{
                    initial_errors: vec![e.to_string()],
                }
            };
        }
    };

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
                (organization.name.clone(), Some(Routes::OrganizationPage { org_id: organization.id.clone() })),
                (event.name.clone(), None),
            ],
            menu: menu,
            div {
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
                        class: highlight.is_active(&MenuItem::Registrations),
                        onclick: move |e| {
                            e.prevent_default();
                            nav.push(Routes::RegistrationPage { id: event_id.read().clone() });
                        },
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
