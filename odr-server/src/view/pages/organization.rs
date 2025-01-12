use crate::{
    hooks::toasts::use_toasts,
    server_functions::{
        event::{query as query_events, upsert as upsert_events},
        organization::query as query_organizations,
        ProtoWrapper,
    },
    view::{
        app::Routes,
        components::{
            form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
            menu::Menu as GenericMenu,
            modal::Modal,
            page::Page as GenericPage,
            table::Table,
            with_toasts::WithToasts,
        },
    },
};
use common::proto::{
    organization_query, string_query, Event, Organization, OrganizationQuery, QueryEventsRequest,
    QueryOrganizationsRequest, StringQuery, UpsertEventsRequest, UpsertOrganizationsResponse,
};
use dioxus::prelude::*;

#[component]
pub fn Page(org_id: ReadOnlySignal<String>) -> Element {
    let organizations_response = use_server_future(move || {
        query_organizations(ProtoWrapper(QueryOrganizationsRequest {
            query: Some(OrganizationQuery {
                query: Some(organization_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(org_id())),
                })),
            }),
        }))
    })?;

    let events_response =
        use_server_future(move || query_events(ProtoWrapper(QueryEventsRequest { query: None })))?;

    let (ProtoWrapper(organizations_response), ProtoWrapper(events_response)) =
        match (organizations_response(), events_response()) {
            (None, _) | (_, None) => return rsx! {},
            (Some(or), Some(er)) => {
                let mut errors = Vec::new();
                if let Err(ref e) = or {
                    errors.push(e.to_string());
                };
                if let Err(ref e) = er {
                    errors.push(e.to_string());
                };

                if !errors.is_empty() {
                    return rsx! {
                        WithToasts {
                            initial_errors: errors,
                        }
                    };
                };

                (or.unwrap(), er.unwrap())
            }
        };

    let org = match organizations_response.organizations.first() {
        Some(org) => org,
        None => {
            return rsx! {
                WithToasts {
                    initial_errors: vec!["Organization not found".to_string()],
                }
            }
        }
    };

    rsx! {
        WithToasts{
            ServerRenderedPage {
                org: org.clone(),
                events: events_response.events,
            }
        }
    }
}

#[component]
fn ServerRenderedPage(org: Organization, events: Vec<Event>) -> Element {
    let mut events = use_signal(move || events);
    let mut show_event_modal = use_signal(|| false);
    let nav = use_navigator();

    let menu = rsx! {
        Menu {
            org_name: org.name.clone(),
            org_id: org.id.clone(),
            highlight: MenuItem::OrganizationHome,
        }
    };

    let event_modal = if *show_event_modal.read() {
        rsx! {
            EventModal {
                org_id: org.id,
                onsubmit: move |event| {
                    show_event_modal.set(false);
                    events.write().push(event);
                },
                onclose: move |_| show_event_modal.set(false),
            }
        }
    } else {
        rsx! {}
    };

    let page_body = rsx! {
        Table {
            is_striped: true,
            is_fullwidth: true,
            thead {
                tr {
                    th {
                        class: "col-auto",
                        "Name"
                    }
                    th{
                        style: "width: 1px",
                    }
                }
            }
            tbody {
                { events.read().iter().map(|e|{
                    let id = e.id.clone();
                    rsx!{
                        tr {
                            key: "{e.id}",
                            td{
                                class: "col-auto",
                                "{e.name}"
                            }
                            td {
                                style: "width: 1px; white-space: nowrap;",
                                Button {
                                    flavor: ButtonFlavor::Info,
                                    onclick: move |_| {
                                        /*nav.push(Routes::EventPage{
                                            id: id.clone(),
                                        });*/
                                    },
                                    "Edit Event"
                                }
                            }
                        }
                    }
                }) }
            }
        }
        Button {
            flavor: ButtonFlavor::Info,
            onclick: move |_| show_event_modal.set(true),
            "Create New Event"
        }
        { event_modal }
    };

    rsx! {
        GenericPage {
            title: org.name.clone(),
            breadcrumb: vec![
                ("Home".to_owned(), Some(Routes::LandingPage)),
                (org.name, None),
            ],
            menu: menu,
            { page_body }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum MenuItem {
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
fn Menu(
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
                        class: highlight.is_active(&MenuItem::OrganizationHome),
                        onclick: move |e| {
                            e.prevent_default();
                            nav.push(Routes::OrganizationPage { org_id: org_id.read().clone() });
                        },
                        "Organization Home"
                    }
                }
            }
        }
    }
}

#[component]
fn EventModal(org_id: String, onsubmit: EventHandler<Event>, onclose: EventHandler<()>) -> Element {
    let mut event_name = use_signal(String::new);
    let mut submitted = use_signal(|| false);
    let mut toaster = use_toasts();

    rsx! {
        Modal{
            onsubmit: move |_| {
                submitted.set(true);

                let org_id = org_id.clone();
                spawn(async move {
                    let rsp = upsert_events(ProtoWrapper(UpsertEventsRequest{
                        events: vec![Event{
                            id: "".to_owned(),
                            organization_id: org_id,
                            name: event_name.read().clone(),
                        }],
                    })).await;

                    match rsp {
                        Ok(ProtoWrapper(mut rsp)) => onsubmit.call(rsp.events.remove(0)),
                        Err(e) => {
                            toaster.write().new_error(e.to_string());
                            submitted.set(false);
                        },
                    }

                });
            },
            onclose: move |_| onclose.call(()),
            disable_submit: *submitted.read(),
            title: "Create new Event",
            success_text: "Create",
            form {
                Field {
                    label: "Event Name",
                    TextInput {
                        oninput: move |evt: FormEvent| event_name.set(evt.value()),
                        value: TextInputType::Text(event_name.read().clone()),
                    }
                }
            }
        }
    }
}
