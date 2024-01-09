use common::proto::ListEventsRequest;
use dioxus::prelude::*;
use tonic::Request;

use crate::hooks::{use_grpc_client, use_grpc_client_provider, EventsClient};

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    use_grpc_client_provider::<EventsClient>(cx);

    let events_client = use_grpc_client::<EventsClient>(cx);

    let event = use_future(cx, (), |_| {
        to_owned!(events_client, id);
        async move {
            match events_client {
                Some(client) => {
                    let response = client
                        .lock()
                        .unwrap()
                        .list_events(Request::new(ListEventsRequest { ids: vec![id] }))
                        .await
                        .unwrap();

                    response.into_inner().events.pop()
                }
                None => None,
            }
        }
    });

    match event.value().map(Option::as_ref).flatten() {
        Some(e) => cx.render(rsx! {
            div {
                class: "container",
                h2{
                    "{e.name}"
                }
                div {
                    class: "row",
                    div {
                        class: "col",
                        button {
                            class: "btn btn-primary",
                            "Edit Registration Schemas",
                        }
                    }
                }
            }
        }),
        None => None,
    }
}
