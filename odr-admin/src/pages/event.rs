use common::proto::ListEventsRequest;
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tonic::Request;

use crate::{
    components::page::Page as GenericPage,
    hooks::{use_grpc_client, use_grpc_client_provider, EventsClient},
    pages::Routes,
};

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

    let nav = use_navigator(cx);

    match event.value().map(Option::as_ref).flatten() {
        Some(e) => cx.render(rsx! {
            GenericPage {
                title: e.name.clone(),
                div {
                    class: "row",
                    div {
                        class: "col",
                        button {
                            class: "btn btn-primary",
                            onclick: |_| {
                                nav.push(Routes::RegistrationSchemaPage {
                                    id: id.clone(),
                                });
                            },
                            "Modify Registration Schema",
                        }
                    }
                }
            }
        }),
        None => None,
    }
}
