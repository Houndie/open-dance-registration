use crate::{
    hooks::{handle_error::use_handle_error, toasts::use_toasts},
    server_functions::{
        authentication::claims,
        user::{query as query_users, upsert as upsert_users},
        ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{
            form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
            menu::Menu as GenericMenu,
            page::Page as GenericPage,
        },
    },
};
use common::proto::{
    string_query, user, user_query, ClaimsRequest, QueryUsersRequest, StringQuery,
    UpsertUsersRequest, User, UserQuery,
};
use dioxus::prelude::*;

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

#[component]
pub fn Page() -> Element {
    let results = use_server_future(move || async move {
        let claims = claims(ClaimsRequest {})
            .await
            .map_err(Error::from_server_fn_error)?
            .claims
            .ok_or(Error::Unauthenticated)?;

        let mut users = query_users(QueryUsersRequest {
            query: Some(UserQuery {
                query: Some(user_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(claims.sub.clone())),
                })),
            }),
        })
        .await
        .map_err(Error::from_server_fn_error)?;

        let user = users.users.pop().ok_or(Error::NotFound)?;

        Ok(ProtoWrapper(user))
    })?;

    use_handle_error(results.suspend()?, |ProtoWrapper(user)| {
        let menu = rsx! {
            Menu {
                user_name: user.display_name.clone(),
                highlight: MenuItem::AccountSettings,
            }
        };

        rsx! {
            GenericPage {
                title: user.display_name.clone(),
                breadcrumb: vec![
                    ("Home".to_owned(), Some(Routes::LandingPage)),
                    (user.display_name.clone(), None),
                ],
                menu: menu,
                PageBody {
                    user: user,
                }
            }
        }
    })
}

#[derive(Clone, Copy, PartialEq)]
pub enum MenuItem {
    None,
    AccountSettings,
}

impl MenuItem {
    fn is_active(&self, this: &MenuItem) -> &'static str {
        if *self == *this {
            "is-active"
        } else {
            ""
        }
    }
}

#[component]
pub fn Menu(user_name: ReadOnlySignal<String>, highlight: Option<MenuItem>) -> Element {
    let nav = use_navigator();
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    rsx! {
        GenericMenu {
            title: "{user_name}",
            p {
                class: "menu-label",
                "Account",
            }
            ul {
                class: "menu-list",
                li {
                    class: highlight.is_active(&MenuItem::AccountSettings),
                    a {
                        onclick: move |_| { nav.push(Routes::ProfilePage); },
                        "Account Settings",
                    }
                }
            }

        }
    }
}

#[component]
fn PageBody(user: ReadOnlySignal<User>) -> Element {
    let mut toaster = use_toasts();
    let mut user_info = use_signal(move || user().clone());

    let mut profile_form: Signal<ProfileForm> = use_signal(|| user().clone().into());
    rsx! {
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

                        let res = upsert_users(UpsertUsersRequest{
                            users: vec![send_user],
                        }).await;

                        let mut response = match res {
                            Ok(res) => res,
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
