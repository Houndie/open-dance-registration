use common::proto::{
    self, event_query, organization_query, string_query, EventQuery, Organization,
    OrganizationQuery, QueryEventsRequest, QueryOrganizationsRequest, StringQuery,
};
use dioxus::prelude::*;
use tonic::Request;

use crate::{
    components::{
        menu::event::{Menu, MenuItem},
        page::Page as GenericPage,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

#[component]
pub fn Page(id: ReadOnlySignal<String>) -> Element {
    let grpc_client = use_grpc_client();

    let mut toaster = use_toasts();
    let nav = use_navigator();

    let resource = use_resource(move || {
        let mut grpc_client = grpc_client.clone();
        async move {
            let result = grpc_client
                .events
                .query_events(Request::new(QueryEventsRequest {
                    query: Some(EventQuery {
                        query: Some(event_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(id.read().clone())),
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

            let event = response.into_inner().events.pop();

            let event = match event {
                Some(event) => event,
                None => {
                    nav.push(Routes::NotFound);
                    return None;
                }
            };

            let result = grpc_client
                .organizations
                .query_organizations(tonic::Request::new(QueryOrganizationsRequest {
                    query: Some(OrganizationQuery {
                        query: Some(organization_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(
                                event.organization_id.clone(),
                            )),
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

            let org = match org {
                Some(org) => org,
                None => {
                    toaster
                        .write()
                        .new_error("Organization not found".to_string());
                    return None;
                }
            };

            rsx! {
                LoadedPage {
                    org: org,
                    event: event,
                }
            }
        }
    });

    match resource() {
        Some(page) => page,
        None => None,
    }
}

#[component]
fn LoadedPage(org: ReadOnlySignal<Organization>, event: ReadOnlySignal<proto::Event>) -> Element {
    rsx! {
        GenericPage {
            title: "Event Home".to_string(),
            breadcrumb: vec![
                ("Home".to_owned(), Some(Routes::OrganizationsPage)),
                (org().name.clone(), Some(Routes::EventsPage { org_id: org().id.clone() })),
                (event().name.clone(), None),
            ],
            menu: rsx!{
                Menu {
                    event_name: event().name,
                    event_id: event().id,
                    highlight: MenuItem::EventHome,
                }
            },
            div {
            }
        }
    }
}
