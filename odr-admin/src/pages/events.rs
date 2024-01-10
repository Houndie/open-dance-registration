use common::proto::{self, ListEventsRequest, UpsertEventsRequest};
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tonic::Status;

use crate::{
    components::page::Page as GenericPage,
    hooks::{use_grpc_client, use_grpc_client_provider, EventsClient},
    pages::Routes,
};

pub fn Page(cx: Scope) -> Element {
    use_grpc_client_provider::<EventsClient>(cx);

    let events_client = use_grpc_client::<EventsClient>(cx);

    let events = use_ref(cx, || Vec::new());

    let rsp: &UseFuture<Result<(), Status>> = use_future(cx, (), |_| {
        to_owned!(events_client, events);
        async move {
            if let Some(client) = events_client {
                let response = client
                    .lock()
                    .unwrap()
                    .list_events(tonic::Request::new(ListEventsRequest { ids: Vec::new() }))
                    .await?;

                events.with_mut(|events| *events = response.into_inner().events);
            }
            Ok(())
        }
    });

    if let Some(err) = rsp.value().map(|rsp| rsp.as_ref().err()).flatten() {
        log::error!("Error occurred while fetching events: {}", err);
    };

    let show_event_modal = use_state(cx, || false);

    let nav = use_navigator(cx);
    cx.render(rsx! {
        GenericPage {
            title: "My Events".to_owned(),
            table {
                class: "table table-striped",
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
                                    button {
                                        class: "btn btn-primary",
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
            button {
                class: "btn btn-primary",
                onclick: move |_| show_event_modal.set(true),
                "Create New Event"
            }
        }
        if **show_event_modal { rsx!(EventModal {
            do_submit: |event| {
                show_event_modal.set(false);
                events.with_mut(|events| events.push(event));
            },
            do_close: || show_event_modal.set(false),
        }) }
    })
}

#[component]
fn Modal<'a, DoSubmit: Fn() -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    do_submit: DoSubmit,
    do_close: DoClose,
    title: &'a str,
    disableSubmit: bool,
    children: Element<'a>,
) -> Element {
    cx.render(rsx! {
        div {
            style: "position: fixed; z-index: 1; padding-top: 100px; left: 0; top: 0; width: 100%; height: 100%; overflow: auto; background-color: rgb(0,0,0); background-color: rgba(0,0,0,0.4);",
            div {
                style: "margin: auto; width: 80%",
                div {
                    class: "card",
                    div {
                        div {
                            class: "card-header",
                            div {
                                class: "row",
                                div {
                                    class: "col",
                                    h5 {
                                        class: "card-title",
                                        "{title}"
                                    }
                                }
                                div {
                                    class: "col-1 d-flex justify-content-end",
                                    button {
                                        class: "btn-close",
                                        onclick: |_| do_close(),
                                    }
                                }
                            }
                        }
                        div {
                            class: "card-body",
                            &children,

                            div {
                                class: "d-flex flex-row-reverse",
                                div {
                                    class: "p-1 flex-shrink-1",
                                    button {
                                        class: "btn btn-primary",
                                        disabled: *disableSubmit,
                                        onclick: |_| {
                                            log::info!("HI");
                                            do_submit()
                                        },
                                        "Create"
                                    }
                                }
                                div {
                                    class: "p-1 flex-shrink-1",
                                    button {
                                        class: "btn btn-secondary",
                                        onclick: |_| do_close(),
                                        "Cancel"
                                    }
                                }
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
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let event_name = use_state(cx, || "".to_owned());
    let submitted = use_state(cx, || false);
    let created = use_ref(cx, || None);
    let client = use_grpc_client::<EventsClient>(cx);

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
                    to_owned!(client, event_name, created);
                    async move {
                        let rsp = client.unwrap().lock().unwrap().upsert_events(UpsertEventsRequest{
                            events: vec![proto::Event{
                                id: "".to_owned(),
                                name: event_name.current().as_ref().clone(),
                            }],
                        }).await.unwrap();
                        created.set(Some(rsp.into_inner().events.remove(0)));
                    }
                })
            },
            do_close: || do_close(),
            disableSubmit: **submitted,
            title: "Create new Event",
            form {
                div {
                    class: "mb-3",
                    label {
                        "for": "create-event-name-input",
                        class: "form-label",
                        "Event Name"
                    }
                    input {
                        id: "create-event-name-input",
                        class: "form-control",
                        value: "{event_name}",
                        oninput: move |evt| event_name.set(evt.value.clone()),
                    }
                }
            }
        }
    })
}
