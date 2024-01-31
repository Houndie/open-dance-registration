use common::proto::{self, QueryEventsRequest, UpsertEventsRequest};
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    components::{
        form::{Button, ButtonFlavor, TextInput, TextInputType},
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client, use_grpc_client_provider, EventsClient},
    pages::Routes,
};

#[component]
pub fn Page(cx: Scope, org_id: String) -> Element {
    use_grpc_client_provider::<EventsClient>(cx);

    let events_client = use_grpc_client::<EventsClient>(cx).unwrap();

    let toast_manager = use_toasts(cx).unwrap();

    let events = use_ref(cx, || Vec::new());

    let rsp: &UseFuture<Result<(), anyhow::Error>> = use_future(cx, (), |_| {
        to_owned!(events_client, events);
        async move {
            let response = events_client
                .lock()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .query_events(tonic::Request::new(QueryEventsRequest { query: None }))
                .await
                .map_err(|e| anyhow::Error::new(e))?;

            events.with_mut(|events| *events = response.into_inner().events);
            Ok(())
        }
    });

    if let Some(err) = rsp.value().map(|rsp| rsp.as_ref().err()).flatten() {
        toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(err.to_string()));
    };

    let show_event_modal = use_state(cx, || false);

    let nav = use_navigator(cx);
    cx.render(rsx! {
        GenericPage {
            title: "My Events".to_owned(),
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
        }
        if **show_event_modal { rsx!(EventModal {
            org_id: org_id.clone(),
            do_submit: |event| {
                show_event_modal.set(false);
                events.with_mut(|events| events.push(event));
            },
            do_close: || show_event_modal.set(false),
        }) }
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
    let client = use_grpc_client::<EventsClient>(cx).unwrap();
    let toast_manager = use_toasts(cx).unwrap();

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
                    submitted.set(true);
                    to_owned!(client, event_name, created, toast_manager, org_id, submitted);
                    async move {
                        let rsp = {
                            let lock = client.lock();
                            match lock {
                                Ok(mut unlocked) => {
                                    let rsp = unlocked.upsert_events(UpsertEventsRequest{
                                        events: vec![proto::Event{
                                            id: "".to_owned(),
                                            organization_id: org_id.clone(),
                                            name: event_name.current().as_ref().clone(),
                                        }],
                                    }).await;

                                    match rsp {
                                        Ok(rsp) => Some(rsp),
                                        Err(e) => {
                                            toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(e.to_string()));
                                            submitted.set(false);
                                            None
                                        },
                                    }
                                },
                                Err(e) =>  {
                                    toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(e.to_string()));
                                    submitted.set(false);
                                    None
                                },
                            }

                        };
                        if let Some(rsp) = rsp {
                            created.set(Some(rsp.into_inner().events.remove(0)));
                        };
                    }
                })
            },
            do_close: || do_close(),
            disable_submit: **submitted,
            title: "Create new Event",
            form {
                TextInput {
                    oninput: move |evt: FormEvent| event_name.set(evt.value.clone()),
                    value: TextInputType::Text(event_name.get().clone()),
                    label: "Event Name",
                    input_id: "create-event-name-input",
                }
            }
        }
    })
}
