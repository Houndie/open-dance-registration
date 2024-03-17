use common::proto::{event_query, string_query, EventQuery, QueryEventsRequest, StringQuery};
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tonic::Request;

use crate::{
    components::{
        form::{Button, ButtonFlavor},
        page::Page as GenericPage,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    let grpc_client = use_grpc_client(cx).unwrap();

    let toaster = use_toasts(cx).unwrap();
    let nav = use_navigator(cx);

    let event_success = use_future(cx, (id,), |(id,)| {
        to_owned!(grpc_client, toaster, nav);
        async move {
            let result = grpc_client
                .events
                .query_events(Request::new(QueryEventsRequest {
                    query: Some(EventQuery {
                        query: Some(event_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(id.clone())),
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

            let evt = response.into_inner().events.pop();

            if evt.is_none() {
                nav.push(Routes::NotFound);
                return None;
            }

            evt
        }
    });

    let event = match event_success.value().map(|o| o.as_ref()).flatten() {
        Some(evt) => evt,
        None => return None,
    };

    cx.render(rsx! {
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
                div {
                    class: "col",
                    Button {
                        flavor: ButtonFlavor::Info,
                        onclick: |_| {
                            nav.push(Routes::RegistrationPage {
                                event_id: id.clone(),
                            });
                        },
                        "View Registrations",
                    }
                }
            }
        }
    })
}
