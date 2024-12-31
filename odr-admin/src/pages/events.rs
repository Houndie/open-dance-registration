use common::proto::{
    self, organization_query, string_query, Organization, OrganizationQuery, QueryEventsRequest,
    QueryOrganizationsRequest, StringQuery, UpsertEventsRequest,
};
use dioxus::prelude::*;

use crate::{
    components::{
        form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
        menu::organization::{Menu, MenuItem},
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

#[component]
pub fn Page(org_id: ReadOnlySignal<String>) -> Element {
    let grpc_client = use_grpc_client();

    let mut toaster = use_toasts();

    let nav = use_navigator();

    let page = use_resource(move || {
        let mut grpc_client = grpc_client.clone();
        async move {
            let result = grpc_client
                .organizations
                .query_organizations(tonic::Request::new(QueryOrganizationsRequest {
                    query: Some(OrganizationQuery {
                        query: Some(organization_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(org_id())),
                        })),
                    }),
                }))
                .await;

            let response = match result {
                Ok(rsp) => rsp,
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    return rsx! {};
                }
            };

            let org = match response.into_inner().organizations.pop() {
                Some(org) => org,
                None => {
                    nav.push(Routes::NotFound);
                    return rsx! {};
                }
            };

            let result = grpc_client
                .events
                .query_events(tonic::Request::new(QueryEventsRequest { query: None }))
                .await;

            let events = match result {
                Ok(rsp) => Some(rsp.into_inner().events),
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    None
                }
            };

            rsx! {
                LoadedPage {
                    org: org,
                    loaded_events: events,
                }
            }
        }
    });

    match page() {
        Some(page) => page,
        None => rsx! {},
    }
}

#[component]
fn LoadedPage(
    org: ReadOnlySignal<Organization>,
    loaded_events: ReadOnlySignal<Option<Vec<proto::Event>>>,
) -> Element {
    let mut events = use_signal(move || loaded_events().map(|x| x.clone()).unwrap_or_default());
    let mut show_event_modal = use_signal(|| false);
    let nav = use_navigator();

    let menu = rsx! {
        Menu {
            org_name: org().name.clone(),
            org_id: org().id.clone(),
            highlight: MenuItem::OrganizationHome,
        }
    };

    let event_modal = if *show_event_modal.read() {
        rsx! {
            EventModal {
                org_id: org.map(|org| &org.id),
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

    let page_body = loaded_events().map(|_| {
        rsx! {
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
                                            nav.push(Routes::EventPage{
                                                id: id.clone(),
                                            });
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
        }
    });

    rsx! {
        GenericPage {
            title: org().name.clone(),
            breadcrumb: vec![
                ("Home".to_owned(), Some(Routes::OrganizationsPage)),
                (org().name, None),
            ],
            menu: menu,
            { page_body }
        }
    }
}

#[component]
fn EventModal(
    org_id: MappedSignal<String>,
    onsubmit: EventHandler<proto::Event>,
    onclose: EventHandler<()>,
) -> Element {
    let mut event_name = use_signal(String::new);
    let mut submitted = use_signal(|| false);
    let client = use_grpc_client();
    let mut toaster = use_toasts();

    rsx! {
        Modal{
            onsubmit: move |_| {
                submitted.set(true);

                let mut client = client.clone();
                let org_id = org_id.clone();
                spawn(async move {
                    let rsp = { client.events.upsert_events(UpsertEventsRequest{
                        events: vec![proto::Event{
                            id: "".to_owned(),
                            organization_id: org_id().clone(),
                            name: event_name.read().clone(),
                        }],
                    })}.await;

                    match rsp {
                        Ok(rsp) => onsubmit.call(rsp.into_inner().events.remove(0)),
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
