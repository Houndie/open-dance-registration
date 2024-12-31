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

pub fn Page() -> Element {
    let login = use_login();

    let grpc = use_grpc_client();
    let mut toaster = use_toasts();

    let page = use_resource(move || {
        let mut grpc = grpc.clone();
        async move {
            let claims = match &*login.read() {
                LoginState::LoggedIn(claims) => claims.clone(),
                LoginState::LoggedOut => {
                    return rsx! {
                        p { "You are not logged in" }
                    }
                }
                LoginState::Unknown => return rsx! {},
            };

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
                    return rsx! {};
                }
            };

            let user = match user {
                Some(user) => user,
                None => {
                    toaster.write().new_error("User not found".to_owned());
                    return rsx! {};
                }
            };

            rsx! {
                LoadedPage {
                    user: user,
                }
            }
        }
    });

    page().unwrap()
}

#[component]
fn LoadedPage(user: ReadOnlySignal<User>) -> Element {
    let grpc = use_grpc_client();
    let mut toaster = use_toasts();
    let mut user_info = use_signal(move || user().clone());

    let mut profile_form: Signal<ProfileForm> = use_signal(|| user().clone().into());
    rsx! {
        GenericPage {
            title: "Profile".to_string(),
            menu: rsx! {
                Menu {
                    user_name: user_info().display_name.clone(),
                    highlight: MenuItem::AccountSettings,
                }
            },
            form {
                Field {
                    label: "Display Name",
                    TextInput {
                        value: TextInputType::Text(profile_form.read().display_name.clone()),
                        oninput: move |v: FormEvent| {
                            profile_form.write().display_name = v.value();
                        },
                    }
                }
                Field {
                    label: "Password",
                    TextInput {
                        value: TextInputType::Password(profile_form.read().password.clone()),
                        oninput: move |v: FormEvent| {
                            profile_form.with_mut(|form| {
                                form.passwords_match = true;
                                form.password = v.value();
                            })
                        },
                        onblur: move |_| {
                            profile_form.with_mut(|form| {
                                form.passwords_match = form.password_confirm == "" || form.password == form.password_confirm;
                            })
                        },
                    }
                }
                Field {
                    label: "Confirm Password",
                    TextInput {
                        value: TextInputType::Password(profile_form.read().password_confirm.clone()),
                        oninput: move |v: FormEvent| {
                            profile_form.with_mut(|form| {
                                form.passwords_match = true;
                                form.password_confirm = v.value();
                            })
                        },
                        onblur: move |_| {
                            profile_form.with_mut(|form| {
                                form.passwords_match = form.password == form.password_confirm;
                            })
                        },
                        invalid: if !profile_form.read().passwords_match {
                            Some("Passwords do not match".to_owned())
                        } else {
                            None
                        },
                    }
                }
                Button {
                    flavor: ButtonFlavor::Info,
                    onclick: move |_| {
                        let mut grpc = grpc.clone();
                        spawn( async move {
                            let send_user = profile_form.with(|form| {
                                let password: Option<user::Password> = if form.password == "" {
                                    Some(user::Password::Unchanged(()))
                                } else {
                                    Some(user::Password::Set(form.password.clone()))
                                };

                                user_info.with(|user| {
                                    User{
                                        id: user.id.clone(),
                                        email: user.email.clone(),
                                        password: password,
                                        display_name: form.display_name.clone(),
                                    }
                                })
                            });

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

                            user_info.set(response.users.pop().unwrap());
                        });
                    },
                    "Save",
                }
            }
        }
    }
}
