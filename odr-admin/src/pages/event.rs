use common::proto::{event_query, string_query, EventQuery, QueryEventsRequest, StringQuery};
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tonic::Request;

use crate::{
    components::{
        form::{Button, ButtonFlavor},
        page::Page as GenericPage,
    },
    hooks::{toasts::use_toasts, use_grpc_client, use_grpc_client_provider, EventsClient},
    pages::Routes,
};

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    use_grpc_client_provider::<EventsClient>(cx);

    let events_client = use_grpc_client::<EventsClient>(cx).unwrap();
    let toast_manager = use_toasts(cx).unwrap();

    let event = use_future(cx, (), |_| {
        to_owned!(events_client, id);
        async move {
            let response = events_client
                .lock()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .query_events(Request::new(QueryEventsRequest {
                    query: Some(EventQuery {
                        query: Some(event_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(id.clone())),
                        })),
                    }),
                }))
                .await
                .map_err(|e| anyhow::Error::new(e))?;

            response
                .into_inner()
                .events
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No event returned, yet no error"))
        }
    });

    let nav = use_navigator(cx);

    match event.value() {
        Some(rsp) => match rsp {
            Ok(event) => cx.render(rsx! {
                GenericPage {
                    title: event.name.clone(),
                    div {
                        class: "row",
                        div {
                            class: "col",
                            Button {
                                flavor: ButtonFlavor::Info,
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
            Err(e) => {
                toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(e.to_string()));
                None
            }
        },
        None => None,
    }
}
