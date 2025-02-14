use crate::{
    hooks::handle_error::use_handle_error,
    proto::{
        compound_user_query, permission_query, permission_role, permission_role_query,
        string_query, user_query, ClaimsRequest, CompoundUserQuery, Permission, PermissionQuery,
        PermissionRole, PermissionRoleQuery, QueryPermissionsRequest, QueryUsersRequest,
        ServerAdminRole, StringQuery, User, UserQuery,
    },
    server_functions::{
        authentication::claims, permission::query as query_permissions, user::query as query_users,
        ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{page::Page as GenericPage, table::Table, with_toasts::WithToasts},
        pages::landing::{Menu, MenuItem},
    },
};
use dioxus::prelude::*;
use std::collections::HashMap;

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

        let user_queries = permissions_response
            .permissions
            .iter()
            .map(|permission| UserQuery {
                query: Some(user_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(permission.user_id.clone())),
                })),
            })
            .collect::<Vec<_>>();

        let users_response = query_users(QueryUsersRequest {
            query: Some(UserQuery {
                query: Some(user_query::Query::Compound(CompoundUserQuery {
                    operator: compound_user_query::Operator::Or as i32,
                    queries: user_queries,
                })),
            }),
        })
        .await
        .map_err(Error::from_server_fn_error)?;

        Ok((
            ProtoWrapper(permissions_response),
            ProtoWrapper(users_response),
        ))
    })?;

    use_handle_error(
        results.suspend()?,
        |(ProtoWrapper(permissions_response), ProtoWrapper(users_response))| {
            let user_map = users_response
                .users
                .iter()
                .map(|user| (user.id.clone(), user.clone()))
                .collect::<HashMap<_, _>>();

            // Check to make sure they all exist
            for permission in permissions_response.permissions.iter() {
                if !user_map.contains_key(&permission.user_id) {
                    return rsx! {
                        WithToasts{
                            initial_errors: vec!["user from permission not found in user listing".to_string()]
                        }
                    };
                }
            }

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
                        user_map: user_map,
                    }
                }
            }
        },
    )
}

#[component]
fn PageBody(
    permissions: ReadOnlySignal<Vec<Permission>>,
    user_map: ReadOnlySignal<HashMap<String, User>>,
) -> Element {
    rsx! {
        h2 {
            class: "title is-2",
            "Permissions"
        }

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
                    let user_map = user_map.read();
                    let user_name = &user_map.get(&permission.user_id).unwrap().display_name;
                    rsx!{
                        tr {
                            key: "{permission.id}",
                            td {
                                "{user_name}",
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
