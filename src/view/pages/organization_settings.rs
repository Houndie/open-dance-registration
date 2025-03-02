use crate::{
    hooks::handle_error::use_handle_error,
    proto::{
        compound_permission_query, compound_user_query, organization_query, permission_query,
        permission_role, permission_role_query, string_query, user_query, ClaimsRequest,
        CompoundPermissionQuery, CompoundUserQuery, OrganizationAdminRole, OrganizationQuery,
        OrganizationViewerRole, Permission, PermissionQuery, PermissionRole, PermissionRoleQuery,
        QueryOrganizationsRequest, QueryPermissionsRequest, QueryUsersRequest, QueryUsersResponse,
        StringQuery, User, UserQuery,
    },
    server_functions::{
        authentication::claims, organization::query as query_organizations,
        permission::query as query_permissions, user::query as query_users, ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{
            page::Page as GenericPage, permissions::PermissionsTable, with_toasts::WithToasts,
        },
        pages::organization::{Menu, MenuItem},
    },
};
use dioxus::prelude::*;
use std::collections::HashMap;

#[component]
pub fn Page(org_id: ReadOnlySignal<String>) -> Element {
    let results = use_server_future(move || async move {
        let claims_future = claims(ClaimsRequest {});

        let organizations_future = query_organizations(QueryOrganizationsRequest {
            query: Some(OrganizationQuery {
                query: Some(organization_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(org_id())),
                })),
            }),
        });

        let permissions_future = query_permissions(QueryPermissionsRequest {
            query: Some(PermissionQuery {
                query: Some(permission_query::Query::Compound(CompoundPermissionQuery {
                    operator: compound_permission_query::Operator::Or as i32,
                    queries: vec![
                        PermissionQuery {
                            query: Some(permission_query::Query::Role(PermissionRoleQuery {
                                operator: Some(permission_role_query::Operator::Is(
                                    PermissionRole {
                                        role: Some(permission_role::Role::OrganizationAdmin(
                                            OrganizationAdminRole {
                                                organization_id: org_id(),
                                            },
                                        )),
                                    },
                                )),
                            })),
                        },
                        PermissionQuery {
                            query: Some(permission_query::Query::Role(PermissionRoleQuery {
                                operator: Some(permission_role_query::Operator::Is(
                                    PermissionRole {
                                        role: Some(permission_role::Role::OrganizationViewer(
                                            OrganizationViewerRole {
                                                organization_id: org_id(),
                                            },
                                        )),
                                    },
                                )),
                            })),
                        },
                    ],
                })),
            }),
        });

        let _ = claims_future.await.map_err(Error::from_server_fn_error)?;

        let organization = organizations_future
            .await
            .map_err(Error::from_server_fn_error)?
            .organizations
            .pop()
            .ok_or(Error::NotFound)?;

        let permissions_response = permissions_future
            .await
            .map_err(Error::from_server_fn_error)?;

        let users_response = if !permissions_response.permissions.is_empty() {
            let user_queries = permissions_response
                .permissions
                .iter()
                .map(|permission| UserQuery {
                    query: Some(user_query::Query::Id(StringQuery {
                        operator: Some(string_query::Operator::Equals(permission.user_id.clone())),
                    })),
                })
                .collect::<Vec<_>>();

            query_users(QueryUsersRequest {
                query: Some(UserQuery {
                    query: Some(user_query::Query::Compound(CompoundUserQuery {
                        operator: compound_user_query::Operator::Or as i32,
                        queries: user_queries,
                    })),
                }),
            })
            .await
            .map_err(Error::from_server_fn_error)?
        } else {
            QueryUsersResponse::default()
        };

        Ok((
            ProtoWrapper(organization),
            ProtoWrapper(permissions_response),
            ProtoWrapper(users_response),
        ))
    })?;

    use_handle_error(
        results.suspend()?,
        |(
            ProtoWrapper(organization),
            ProtoWrapper(permissions_response),
            ProtoWrapper(users_response),
        )| {
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
                    org_id: org_id,
                    org_name: organization.name,
                    highlight: MenuItem::OrganizationSettings,
                }
            };

            rsx! {
                GenericPage {
                    title: "Server Settings".to_owned(),
                    breadcrumb: vec![
                        ("Home".to_owned(), Some(Routes::LandingPage)),
                        ("Organization Settings".to_owned(), None)
                    ],
                    menu: menu,
                    PageBody{
                        organization_id: org_id,
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
    organization_id: ReadOnlySignal<String>,
    permissions: Vec<Permission>,
    user_map: HashMap<String, User>,
) -> Element {
    rsx! {
        PermissionsTable {
            permissions: permissions,
            user_map: user_map,
            role_options: vec![
                PermissionRole{
                    role: Some(permission_role::Role::OrganizationAdmin(
                        OrganizationAdminRole {
                            organization_id: organization_id(),
                        },
                    )),
                },
                PermissionRole{
                    role: Some(permission_role::Role::OrganizationViewer(
                        OrganizationViewerRole {
                            organization_id: organization_id(),
                        },
                    )),
                },
            ],
            default_role: 1,
        }
    }
}
