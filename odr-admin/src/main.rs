#![allow(non_snake_case)]
use std::sync::{Arc, Mutex};

use common::proto::{
    self, event_service_client::EventServiceClient, ListEventsRequest, UpsertEventsRequest,
};
use dioxus::prelude::*;
use log::LevelFilter;

fn main() {
    dioxus_logger::init(LevelFilter::Info).expect("failed to init logger");
    dioxus_web::launch(App);
}

struct EventsClientContext(Arc<Mutex<EventServiceClient<tonic_web_wasm_client::Client>>>);

fn App(cx: Scope) -> Element {
    use_shared_state_provider(cx, || {
        EventsClientContext(Arc::new(Mutex::new(EventServiceClient::new(
            tonic_web_wasm_client::Client::new("http://localhost:50051".to_owned()),
        ))))
    });

    let events_client = use_events_client(cx);

    let events = use_future(cx, (), |_| {
        to_owned!(events_client);
        async move {
            if let Some(client) = events_client {
                let response = client
                    .lock()
                    .unwrap()
                    .list_events(tonic::Request::new(ListEventsRequest { ids: Vec::new() }))
                    .await
                    .unwrap();

                return response.into_inner().events;
            }

            Vec::new()
        }
    });

    let show_event_modal = use_state(cx, || false);
    cx.render(rsx! {
        div {
            class: "container",
            h2 {
                "My Events"
            }

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
                    match events.value() {
                        Some(events) => rsx! {
                            events.iter().map(|e| rsx!{
                                tr {
                                    td{
                                        class: "col-auto",
                                        e.name.clone()
                                    }
                                    td {
                                        style: "width: 1px; white-space: nowrap;",
                                        button {
                                            class: "btn btn-primary",
                                            "Edit Event"
                                        }
                                    }
                                }
                            })
                        },
                        None => rsx! { tr{} },
                    }
                }
            }
        }
        button {
            class: "btn btn-primary",
            onclick: move |_| show_event_modal.set(true),
            "Create New Event"
        }
        if **show_event_modal { rsx!(EventModal {
            do_submit: || {
                show_event_modal.set(false);
                events.restart();
            },
            do_close: || show_event_modal.set(false),
        }) }
    })
}

fn use_events_client(
    cx: &ScopeState,
) -> Option<Arc<Mutex<EventServiceClient<tonic_web_wasm_client::Client>>>> {
    use_shared_state::<EventsClientContext>(cx).map(|state| state.read().0.clone())
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
fn EventModal<DoSubmit: Fn() -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let event_name = use_state(cx, || "".to_owned());
    let submitted = use_state(cx, || false);
    let created = use_state(cx, || false);
    let client = use_events_client(cx);

    if **created {
        do_submit()
    }

    cx.render(rsx! {
        Modal{
            do_submit: move || {
                cx.spawn({
                    submitted.set(true);
                    to_owned!(client, event_name, created);
                    async move {
                        client.unwrap().lock().unwrap().upsert_events(UpsertEventsRequest{
                            events: vec![proto::Event{
                                id: "".to_owned(),
                                name: event_name.current().as_ref().clone(),
                            }],
                        }).await.unwrap();
                        created.set(true);
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
