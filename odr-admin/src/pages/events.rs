use common::proto::{
    self, organization_query, string_query, OrganizationQuery, QueryEventsRequest,
    QueryOrganizationsRequest, StringQuery, UpsertEventsRequest,
};
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    components::{
        form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

#[component]
pub fn Page(cx: Scope, org_id: String) -> Element {
    let grpc_client = use_grpc_client(cx).unwrap();

    let toaster = use_toasts(cx).unwrap();

    let nav = use_navigator(cx);

    let organizations_rsp = use_future(cx, (), |_| {
        to_owned!(grpc_client, org_id, nav, toaster);
        async move {
            let result = grpc_client
                .organizations
                .query_organizations(tonic::Request::new(QueryOrganizationsRequest {
                    query: Some(OrganizationQuery {
                        query: Some(organization_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(org_id)),
                        })),
                    }),
                }))
                .await;

            let response = match result {
                Ok(rsp) => rsp,
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    return None;
                }
            };

            let org = response.into_inner().organizations.pop();

            if org.is_none() {
                nav.push(Routes::NotFound);
                return None;
            }

            org
        }
    });

    let org = match organizations_rsp.value().map(|o| o.as_ref()).flatten() {
        Some(org) => org,
        None => return None,
    };

    let events = use_ref(cx, || Vec::new());

    let events_rsp: &UseFuture<bool> = use_future(cx, (), |_| {
        to_owned!(grpc_client, events, toaster);
        async move {
            let result = grpc_client
                .events
                .query_events(tonic::Request::new(QueryEventsRequest { query: None }))
                .await;

            let response = match result {
                Ok(rsp) => rsp,
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    return false;
                }
            };

            *events.write() = response.into_inner().events;
            true
        }
    });

    let show_event_modal = use_state(cx, || false);
    cx.render(rsx! {
        GenericPage {
            title: org.name.clone(),
            if matches!(events_rsp.value(), Some(true)) {
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
                            events.read().iter().map(|e|{
                                let id = e.id.clone();
                                rsx!{
                                    tr {
                                        key: "{e.id}",
                                        td{
                                            class: "col-auto",
                                            e.name.clone()
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
                            })
                        }
                    }
                    Button {
                        flavor: ButtonFlavor::Info,
                        onclick: move |_| show_event_modal.set(true),
                        "Create New Event"
                    }
                    if **show_event_modal {
                        rsx!{
                            EventModal {
                                org_id: org_id.clone(),
                                do_submit: |event| {
                                    show_event_modal.set(false);
                                    events.write().push(event);
                                },
                                do_close: || show_event_modal.set(false),
                            }
                        }
                    }
                }
            }
        }
    })
}

#[component]
fn EventModal<DoSubmit: Fn(proto::Event) -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    org_id: String,
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let event_name = use_state(cx, || "".to_owned());
    let submitted = use_state(cx, || false);
    let created = use_ref(cx, || None);
    let client = use_grpc_client(cx).unwrap();
    let toaster = use_toasts(cx).unwrap();

    {
        let mut created_mut = created.write_silent();
        if let Some(event) = created_mut.as_mut() {
            created.needs_update();
            let event = std::mem::take::<proto::Event>(event);
            do_submit(event);
        }
    }

    cx.render(rsx! {
        Modal{
            do_submit: move || {
                cx.spawn({
                    to_owned!(client, event_name, created, toaster, org_id, submitted);
                    async move {
                        submitted.set(true);

                        let rsp = { client.events.upsert_events(UpsertEventsRequest{
                            events: vec![proto::Event{
                                id: "".to_owned(),
                                organization_id: org_id.clone(),
                                name: event_name.get().clone(),
                            }],
                        })}.await;

                        match rsp {
                            Ok(rsp) => created.set(Some(rsp.into_inner().events.remove(0))),
                            Err(e) => {
                                toaster.write().new_error(e.to_string());
                                submitted.set(false);
                            },
                        }

                    }
                })
            },
            do_close: || do_close(),
            disable_submit: **submitted,
            title: "Create new Event",
            success_text: "Create",
            form {
                Field {
                    label: "Event Name",
                    TextInput {
                        oninput: move |evt: FormEvent| event_name.set(evt.value.clone()),
                        value: TextInputType::Text(event_name.get().clone()),
                    }
                }
            }
        }
    })
}
