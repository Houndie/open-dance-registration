use crate::{
    hooks::handle_error::use_handle_error,
    proto::{
        permission_query, permission_role, permission_role_query, ClaimsRequest, Permission,
        PermissionQuery, PermissionRole, PermissionRoleQuery, QueryPermissionsRequest,
        ServerAdminRole,
    },
    server_functions::{
        authentication::claims, permission::query as query_permissions, ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{page::Page as GenericPage, table::Table},
        pages::landing::{Menu, MenuItem},
    },
};
use dioxus::prelude::*;

#[component]
pub fn Page() -> Element {
    let results = use_server_future(|| async {
        let claims_future = claims(ClaimsRequest {});
        let permissions_future = query_permissions(QueryPermissionsRequest {
            query: Some(PermissionQuery {
                query: Some(permission_query::Query::Role(PermissionRoleQuery {
                    operator: Some(permission_role_query::Operator::Is(PermissionRole {
                        role: Some(permission_role::Role::ServerAdmin(ServerAdminRole {})),
                    })),
                })),
            }),
        });

        let _ = claims_future.await.map_err(Error::from_server_fn_error)?;
        let permissions_response = permissions_future
            .await
            .map_err(Error::from_server_fn_error)?;

        Ok(ProtoWrapper(permissions_response))
    })?;

    use_handle_error(results.suspend()?, |ProtoWrapper(permissions_response)| {
        let menu = rsx! {
            Menu {
                highlight: MenuItem::ServerSettings,
            }
        };

        rsx! {
            GenericPage {
                title: "Server Settings".to_owned(),
                breadcrumb: vec![
                    ("Home".to_owned(), Some(Routes::LandingPage)),
                    ("Server Settings".to_owned(), None)
                ],
                menu: menu,
                PageBody{
                    permissions: permissions_response.permissions,
                }
            }
        }
    })
}

#[component]
fn PageBody(permissions: ReadOnlySignal<Vec<Permission>>) -> Element {
    rsx! {
        Table {
            is_striped: true,
            is_fullwidth: true,
            thead {
                tr {
                    th {
                        "User",
                    }
                    th {
                        "Role",
                    }
                }
            }
            tbody {
                { permissions.read().iter().map(|permission| {
                    rsx!{
                        tr {
                            key: "{permission.id}",
                            td {
                                "placeholder",
                            }
                            td {
                                "Server Admin",
                            }
                        }
                    }
                })}
            }
        }
    }
}
