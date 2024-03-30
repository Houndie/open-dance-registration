use common::proto::{
    string_query, user, user_query, QueryUsersRequest, StringQuery, UpsertUsersRequest, User,
    UserQuery,
};
use dioxus::prelude::*;
use tonic::Request;

use crate::{
    components::{
        form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
        menu::profile::{Menu, MenuItem},
        page::Page as GenericPage,
    },
    hooks::{
        login::{use_login, LoginState},
        toasts::use_toasts,
        use_grpc_client,
    },
};

struct ProfileForm {
    display_name: String,
    password: String,
    password_confirm: String,
    passwords_match: bool,
}

impl From<User> for ProfileForm {
    fn from(user: User) -> Self {
        Self {
            display_name: user.display_name,
            password: "".to_owned(),
            password_confirm: "".to_owned(),
            passwords_match: true,
        }
    }
}

pub fn Page(cx: Scope) -> Element {
    let login = use_login(cx).unwrap();
    let claims = match &login.read().0 {
        LoginState::LoggedIn(claims) => claims.clone(),
        LoginState::LoggedOut => {
            return cx.render(rsx! {
                p { "You are not logged in" }
            })
        }
        LoginState::Unknown => return None,
    };

    let grpc = use_grpc_client(cx).unwrap();
    let toaster = use_toasts(cx).unwrap();

    let user_info: &UseRef<Option<common::proto::User>> = use_ref(cx, || None);
    let success = use_future(cx, (&claims,), |(claims,)| {
        to_owned!(grpc, toaster, user_info);
        async move {
            let res = grpc
                .user
                .query_users(Request::new(QueryUsersRequest {
                    query: Some(UserQuery {
                        query: Some(user_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(claims.sub.clone())),
                        })),
                    }),
                }))
                .await;

            let user = match res {
                Ok(res) => res.into_inner().users.pop(),
                Err(e) => {
                    toaster
                        .write()
                        .new_error(format!("Failed to query user: {}", e));
                    return false;
                }
            };

            let user = match user {
                Some(user) => user,
                None => {
                    toaster.write().new_error("User not found".to_owned());
                    return false;
                }
            };

            *user_info.write() = Some(user);
            true
        }
    });

    if !*success.value().unwrap_or(&false) {
        return None;
    }

    let form: &UseRef<ProfileForm> =
        use_ref(cx, || user_info.read().as_ref().cloned().unwrap().into());

    cx.render(rsx! {
        GenericPage {
            title: "Profile".to_string(),
            menu: cx.render(rsx! {
                Menu {
                    user_name: user_info.read().as_ref().unwrap().display_name.clone(),
                    highlight: MenuItem::AccountSettings,
                }
            }),
            form {
                Field {
                    label: "Display Name",
                    TextInput {
                        value: TextInputType::Text(form.read().display_name.clone()),
                        oninput: |v: FormEvent| {
                            form.write().display_name = v.value.clone();
                        },
                    }
                }
                Field {
                    label: "Password",
                    TextInput {
                        value: TextInputType::Password(form.read().password.clone()),
                        oninput: |v: FormEvent| {
                            form.with_mut(|form| {
                                form.passwords_match = true;
                                form.password = v.value.clone();
                            })
                        },
                        onblur: |_| {
                            form.with_mut(|form| {
                                form.passwords_match = form.password_confirm == "" || form.password == form.password_confirm;
                            })
                        },
                    }
                }
                Field {
                    label: "Confirm Password",
                    TextInput {
                        value: TextInputType::Password(form.read().password_confirm.clone()),
                        oninput: |v: FormEvent| {
                            form.with_mut(|form| {
                                form.passwords_match = true;
                                form.password_confirm = v.value.clone();
                            })
                        },
                        onblur: |_| {
                            form.with_mut(|form| {
                                form.passwords_match = form.password == form.password_confirm;
                            })
                        },
                        invalid: if !form.read().passwords_match {
                            Some("Passwords do not match".to_owned())
                        } else {
                            None
                        },
                    }
                }
                Button {
                    flavor: ButtonFlavor::Info,
                    onclick: |_| {
                        cx.spawn({
                            to_owned!(grpc, toaster, form, user_info);
                            async move {
                                let send_user = form.with(|form| {
                                    let password: Option<user::Password> = if form.password == "" {
                                        Some(user::Password::Unchanged(()))
                                    } else {
                                        Some(user::Password::Set(form.password.clone()))
                                    };

                                    user_info.with(|user| {
                                        let user = user.as_ref().unwrap();
                                        User{
                                            id: user.id.clone(),
                                            email: user.email.clone(),
                                            password: password,
                                            display_name: form.display_name.clone(),
                                        }
                                    })
                                });

                                log::info!("Sending user: {:?}", send_user);

                                let res = grpc.user.upsert_users(Request::new(UpsertUsersRequest{
                                    users: vec![send_user],
                                })).await;

                                let mut response = match res {
                                    Ok(res) => res.into_inner(),
                                    Err(e) => {
                                        toaster.write().new_error(format!("Failed to update user info: {}", e));
                                        return;
                                    }
                                };

                                *user_info.write().as_mut().unwrap() = response.users.pop().unwrap();
                            }
                        })
                    },
                    "Save",
                }
            }
        }
    })
}
