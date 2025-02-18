use crate::{
    hooks::{handle_error::use_handle_error, toasts::use_toasts},
    proto::{
        compound_user_query, permission_query, permission_role, permission_role_query,
        string_query, user_query, ClaimsRequest, CompoundUserQuery, DeletePermissionsRequest,
        Permission, PermissionQuery, PermissionRole, PermissionRoleQuery, QueryPermissionsRequest,
        QueryUsersRequest, ServerAdminRole, StringQuery, UpsertPermissionsRequest, User, UserQuery,
    },
    server_functions::{
        authentication::claims,
        permission::{
            delete as delete_permissions, query as query_permissions, upsert as upsert_permissions,
        },
        user::query as query_users,
        ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{
            form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
            modal::Modal,
            page::Page as GenericPage,
            table::Table,
            with_toasts::WithToasts,
        },
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
    let mut permissions = use_signal(|| permissions.read().clone());
    let mut user_map = use_signal(|| user_map.read().clone());
    let mut show_permission_modal: Signal<Option<Option<(Permission, User)>>> = use_signal(|| None);

    let permission_modal = match show_permission_modal.read().as_ref() {
        None => rsx! {},
        Some(permission) => rsx! {
            AddPermissionModal {
                onsubmit: move |(permission, user): (Permission, User)| {
                    show_permission_modal.set(None);
                    permissions.write().push(permission);
                    user_map.write().insert(user.id.clone(), user);
                },
                onclose: move |_| show_permission_modal.set(None),
                ondelete: move |id| {
                    permissions.with_mut(|p| match p.iter().position(|permission| permission.id == id) {
                        Some(idx) => {
                            p.remove(idx);
                        },
                        None => (),
                    });
                    show_permission_modal.set(None);
                },
                permission: permission.clone(),
            },
        },
    };

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
                    let permission = permission.clone();
                    let user_map = user_map.read();
                    let user = user_map.get(&permission.user_id).unwrap().clone();
                    let user_name = user.display_name.clone();
                    rsx!{
                        tr {
                            key: "{permission.id}",
                            td {
                                a {
                                    onclick: move |_| show_permission_modal.set(Some(Some((permission.clone(), user.clone())))),
                                    "{user_name}",
                                }
                            }
                            td {
                                "Server Admin",
                            }
                        }
                    }
                })}
            }
        }
        Button {
            flavor: ButtonFlavor::Info,
            onclick: move |_| show_permission_modal.set(Some(None)),
            "Add User"
        }
        { permission_modal }
    }
}

#[derive(Default)]
struct AddPermissionFormState {
    display_name: String,
}

#[component]
fn AddPermissionModal(
    permission: Option<(Permission, User)>,
    onsubmit: EventHandler<(Permission, User)>,
    ondelete: EventHandler<String>,
    onclose: EventHandler<()>,
) -> Element {
    let mut form_state = use_signal(|| match &permission {
        Some((_, user)) => AddPermissionFormState {
            display_name: user.display_name.clone(),
        },
        None => AddPermissionFormState::default(),
    });
    let mut submitted = use_signal(|| false);
    let mut toaster = use_toasts();

    let title = match &permission {
        Some((_, user)) => user.display_name.clone(),
        None => "Add User".to_string(),
    };

    let remove_button = match &permission {
        Some((permission, _)) => {
            let id = permission.id.clone();
            rsx! {
                Button {
                    flavor: ButtonFlavor::Danger,
                    disabled: *submitted.read(),
                    onclick: move |_| {
                        let id = id.clone();
                        submitted.set(true);
                        spawn(async move {
                            let response = delete_permissions(DeletePermissionsRequest{
                                ids: vec![id.clone()],
                            }).await;

                            if let Err(err) = response {
                                toaster.write().new_error(format!("Error deleting permission: {}", err));
                                submitted.set(false);
                                return;
                            }

                            ondelete.call(id);
                        });
                    },
                    "Remove"
                }
            }
        }
        None => rsx! {},
    };

    rsx! {
        Modal {
            onsubmit: move |_| {
                submitted.set(true);
                spawn( async move {
                    let user_response= query_users(QueryUsersRequest {
                        query: Some(UserQuery {
                            query: Some(user_query::Query::DisplayName(StringQuery {
                                operator: Some(string_query::Operator::Equals(form_state.read().display_name.clone())), })),
                        }),
                    }).await;

                    let user = match user_response {
                        Ok(user_response) => match user_response.users.into_iter().next() {
                            Some(user) => user,
                            None => {
                                toaster.write().new_error("User not found".to_string());
                                submitted.set(false);
                                return;
                            }
                        },
                        Err(err) => {
                            toaster.write().new_error(format!("Error querying users: {}", err));
                            submitted.set(false);
                            return
                        }
                    };

                    let permission_response = upsert_permissions(UpsertPermissionsRequest {
                        permissions: vec![Permission {
                            id: "".to_owned(),
                            user_id: user.id.clone(),
                            role: Some(PermissionRole{
                                role: Some(permission_role::Role::ServerAdmin(ServerAdminRole {})),
                            }),
                        }],
                    }).await;

                    let permission = match permission_response {
                        Ok(permission_response) => permission_response.permissions.into_iter().next().unwrap(),
                        Err(err) => {
                            toaster.write().new_error(format!("Error upserting permissions: {}", err));
                            submitted.set(false);
                            return
                        }
                    };

                    onsubmit.call((permission, user));
                });
            },
            onclose: onclose,
            title: "{title}",
            success_text: "Add",
            disable_submit: *submitted.read(),
            form {
                div {
                    class: "mb-3",
                    Field {
                        label: "Display Name",
                        TextInput {
                            value: TextInputType::Text(form_state.read().display_name.clone()),
                            oninput: move |evt: FormEvent| form_state.write().display_name = evt.value(),
                        }
                    }
                }
            }
            { remove_button }
        }
    }
}
