use crate::{
    hooks::toasts::use_toasts,
    proto::{
        permission_role, string_query, user_query, DeletePermissionsRequest, Permission,
        PermissionRole, QueryUsersRequest, ServerAdminRole, StringQuery, UpsertPermissionsRequest,
        User, UserQuery,
    },
    server_functions::{
        permission::{delete as delete_permissions, upsert as upsert_permissions},
        user::query as query_users,
    },
    view::components::{
        form::{Button, ButtonFlavor, Field, SelectInput, TextInput, TextInputType},
        modal::Modal,
        table::Table,
    },
};
use dioxus::prelude::*;
use std::collections::HashMap;

#[component]
pub fn PermissionsTable(
    permissions: ReadOnlySignal<Vec<Permission>>,
    user_map: ReadOnlySignal<HashMap<String, User>>,
    role_options: ReadOnlySignal<Vec<PermissionRole>>,
    default_role: usize,
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
                role_options: role_options.clone(),
                default_role: default_role,
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
                    let user_name = user.username.clone();
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

fn role_to_string(role: &PermissionRole) -> &'static str {
    match role.role.as_ref().unwrap() {
        permission_role::Role::ServerAdmin(_) => "Server Admin",
        permission_role::Role::OrganizationAdmin(_) => "Organization Admin",
        permission_role::Role::OrganizationViewer(_) => "Organization Viewer",
        permission_role::Role::EventAdmin(_) => "Event Admin",
        permission_role::Role::EventEditor(_) => "Event Editor",
        permission_role::Role::EventViewer(_) => "Event Viewer",
    }
}

#[derive(PartialEq, Clone)]
struct AddPermissionFormState {
    username: String,
    permission_type: usize,
    organization_id: String,
    event_id: String,
}

#[component]
fn AddPermissionModal(
    permission: ReadOnlySignal<Option<(Permission, User)>>,
    onsubmit: EventHandler<(Permission, User)>,
    ondelete: EventHandler<String>,
    onclose: EventHandler<()>,
    role_options: ReadOnlySignal<Vec<PermissionRole>>,
    default_role: usize,
) -> Element {
    let mut toaster = use_toasts();

    let starting_state = use_memo(move || match permission.read().as_ref() {
        Some((permission, user)) => Ok(AddPermissionFormState {
            username: user.username.clone(),
            permission_type: role_options
                .read()
                .iter()
                .position(|r| r == permission.role.as_ref().unwrap())
                .unwrap(),
            organization_id: "".to_string(),
            event_id: "".to_string(),
        }),
        None => Ok(AddPermissionFormState {
            username: "".to_string(),
            permission_type: default_role,
            organization_id: "".to_string(),
            event_id: "".to_string(),
        }),
    });

    if let Err(e) = starting_state() {
        toaster.write().new_error(e);
        return rsx! {};
    }

    let mut form_state = use_signal(|| starting_state().unwrap());
    let mut submitted = use_signal(|| false);

    let title = match permission.read().as_ref() {
        Some((_, user)) => user.username.clone(),
        None => "Add User".to_string(),
    };

    let remove_button = match permission.read().as_ref() {
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
                            query: Some(user_query::Query::Username(StringQuery {
                                operator: Some(string_query::Operator::Equals(form_state.read().username.clone())), })),
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
                        label: "User Name",
                        TextInput {
                            value: TextInputType::Text(form_state.read().username.clone()),
                            oninput: move |evt: FormEvent| form_state.write().username = evt.value(),
                        }
                    }
                    Field {
                        label: "Role",
                        SelectInput {
                            onchange: move |evt: FormEvent| {
                                form_state.write().permission_type = evt.value().parse::<usize>().unwrap();
                            },
                            options: role_options.read().iter().map(|r| role_to_string(r).to_owned()).collect::<Vec<_>>(),
                            value: form_state.read().permission_type,
                        }
                    }
                }
            }
            { remove_button }
        }
    }
}
