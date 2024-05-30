use dioxus::prelude::*;

use crate::{
    components::form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
    hooks::{
        login::{use_login, LoginState},
        toasts::use_toasts,
        use_grpc_client,
    },
    pages::Routes,
};

use common::proto::{LoginRequest, LogoutRequest};

#[derive(Default)]
struct LoginForm {
    username: String,
    password: String,
}

#[component]
pub fn LoginMenu() -> Element {
    let mut show_menu = use_signal(|| false);
    let menu_is_active = if *show_menu.read() { "is-active" } else { "" };

    let mut login_form = use_signal(LoginForm::default);

    let mut toaster = use_toasts();
    let grpc = use_grpc_client();
    let mut is_logged_in = use_login();
    let nav = use_navigator();

    rsx! {
        div {
            class: "dropdown {menu_is_active} is-right",
            div {
                class: "dropdown-trigger",
                button {
                    class: "button",
                    onclick: move |_| {
                        let new_show_menu = !(&*show_menu.read());
                        show_menu.set(new_show_menu);
                    },
                    span{
                        "Login/Signup"
                    }
                    span {
                        class: "icon is-small",
                        "⌄"
                    }
                }
            }
            div {
                class: "dropdown-menu",
                div {
                    class: "dropdown-content",
                    div {
                        class: "dropdown-item",
                        style: "min-width: 300px",
                        { match &*is_logged_in.read() {
                            LoginState::LoggedIn(_) => rsx! {
                                div {
                                    class: "menu",
                                    ul {
                                        class: "menu-list",
                                        li {
                                            a {
                                                prevent_default: "onclick",
                                                onclick: move |_| { nav.push(Routes::ProfilePage); },
                                                "My Profile"
                                            }
                                        }
                                        li {
                                            a {
                                                prevent_default: "onclick",
                                                onclick: move |_| {
                                                    spawn({
                                                        let mut grpc = grpc.clone();
                                                        async move {
                                                            if let Err(e) = grpc.authentication.logout(LogoutRequest{}).await {
                                                                toaster.write().new_error(e.to_string());
                                                                return
                                                            }
                                                            *is_logged_in.write() = LoginState::LoggedOut;
                                                            show_menu.set(false);
                                                        }
                                                    });
                                                },
                                                "Logout"
                                            }
                                        }
                                    }
                                }
                            },
                            LoginState::LoggedOut => rsx! {
                                strong {
                                    "Login"
                                }
                                form {
                                    Field {
                                        label: "Username",
                                        TextInput{
                                            oninput: move |e: FormEvent| {
                                                login_form.write().username = e.value();
                                            },
                                            value: TextInputType::Text(login_form.read().username.clone()),
                                        }
                                    }
                                    Field {
                                        label: "Password",
                                        TextInput{
                                            oninput: move |e: FormEvent| {
                                                login_form.write().password = e.value();
                                            },
                                            value: TextInputType::Password(login_form.read().password.clone()),
                                        }
                                    }
                                    Button {
                                        onclick: move |_| {
                                            spawn({
                                                let mut grpc = grpc.clone();
                                                async move {
                                                    let claims = match grpc.authentication.login(login_form.with(|login_form| {
                                                        LoginRequest {
                                                            email: login_form.username.clone(),
                                                            password: login_form.password.clone(),
                                                        }
                                                    })).await {
                                                        Ok(response) => response.into_inner().claims,
                                                        Err(e) => {
                                                            toaster.write().new_error(e.to_string());
                                                            return
                                                        },
                                                    };

                                                    let claims = match claims {
                                                        Some(claims) => claims,
                                                        None => {
                                                            toaster.write().new_error("No Claims Found".to_string());
                                                            return
                                                        }
                                                    };

                                                    *is_logged_in.write() = LoginState::LoggedIn(claims);
                                                    show_menu.set(false);
                                                }
                                            });
                                        },
                                        flavor: ButtonFlavor::Success,
                                        "Login"
                                    },
                                }
                            },
                            LoginState::Unknown => rsx! {
                                "Loading..."
                            },
                        } }
                    }
                }
            }
        }
    }
}
