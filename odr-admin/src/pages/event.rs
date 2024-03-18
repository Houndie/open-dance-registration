use common::proto::{
    event_query, organization_query, string_query, EventQuery, OrganizationQuery,
    QueryEventsRequest, QueryOrganizationsRequest, StringQuery,
};
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tonic::Request;

use crate::{
    components::{
        form::{Button, ButtonFlavor},
        menu::event::{Menu, MenuItem},
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

    let org_id = &event.organization_id;

    let org = use_future(cx, (), |_| {
        to_owned!(grpc_client, org_id, toaster);
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
                toaster
                    .write()
                    .new_error("Organization not found".to_string());
                return None;
            }

            org
        }
    });

    let org = match org.value().map(|o| o.as_ref()).flatten() {
        Some(org) => org,
        None => return None,
    };

    cx.render(rsx! {
        GenericPage {
            title: "Event Home".to_string(),
            breadcrumb: vec![
                ("Home".to_owned(), Some(Routes::OrganizationsPage)),
                (org.name.clone(), Some(Routes::EventsPage { org_id: org.id.clone() })),
                (event.name.clone(), None),
            ],
            menu: cx.render(rsx!{
                Menu {
                    event_name: event.name.clone(),
                    event_id: event.id.clone(),
                    highlight: MenuItem::EventHome,
                }
            }),
            div {
            }
        }
    })
}
