use crate::{
    hooks::handle_error::use_handle_error,
    proto::{
        event_query, organization_query, string_query, ClaimsRequest, EventQuery,
        OrganizationQuery, QueryEventsRequest, QueryOrganizationsRequest, StringQuery,
    },
    server_functions::{
        authentication::claims, event::query as query_events,
        organization::query as query_organizations, ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{menu::Menu as GenericMenu, page::Page as GenericPage},
    },
};
use dioxus::prelude::*;

#[component]
pub fn Page(id: ReadOnlySignal<String>) -> Element {
    let results = use_server_future(move || async move {
        let claims_future = claims(ClaimsRequest {});

        let events_future = query_events(QueryEventsRequest {
            query: Some(EventQuery {
                query: Some(event_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(id.read().clone())),
                })),
            }),
        });

        let _ = claims_future.await.map_err(Error::from_server_fn_error)?;

        let mut events_response = events_future.await.map_err(Error::from_server_fn_error)?;

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
        .map_err(Error::from_server_fn_error)?;

        let organization = organizations_response
            .organizations
            .pop()
            .ok_or(Error::Misc("organization not found".to_owned()))?;

        Ok((ProtoWrapper(organization), ProtoWrapper(event)))
    })?;

    use_handle_error(
        results.suspend()?,
        |(ProtoWrapper(organization), ProtoWrapper(event))| {
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
        },
    )
}

#[derive(Clone, Copy, PartialEq)]
pub enum MenuItem {
    None,
    EventHome,
    RegistrationSchema,
    Registrations,
    Settings,
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
                li {
                    a {
                        class: highlight.is_active(&MenuItem::Settings),
                        onclick: move |e| {
                            e.prevent_default();
                            nav.push(Routes::EventSettings { event_id: event_id.read().clone() });
                        },
                        "Event Settings"
                    }
                }
            }
        }
    }
}
