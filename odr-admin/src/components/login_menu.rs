use dioxus::prelude::*;

use crate::{
    components::form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
    hooks::{
        login::{use_login, Login},
        toasts::use_toasts,
        use_grpc_client,
    },
};

use common::proto::{LoginRequest, LogoutRequest};

#[derive(Default)]
struct LoginForm {
    username: String,
    password: String,
}

#[component]
pub fn LoginMenu(cx: Scope) -> Element {
    let show_menu = use_state(cx, || false);
    let menu_is_active = if *show_menu.get() { "is-active" } else { "" };

    let login_form = use_ref(cx, LoginForm::default);

    let toaster = use_toasts(cx).unwrap();
    let grpc = use_grpc_client(cx).unwrap();
    let is_logged_in = use_login(cx).unwrap();
    cx.render(rsx! {
        div {
            class: "dropdown {menu_is_active} is-right",
            div {
                class: "dropdown-trigger",
                button {
                    class: "button",
                    onclick: move |_| {
                        show_menu.set(!show_menu.get());
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
                        match is_logged_in.read().0 {
                            Some(_) => rsx! {
                                Button{
                                    onclick: move |_| {
                                        cx.spawn({
                                            to_owned!(show_menu, is_logged_in, toaster, grpc);
                                            async move {
                                                if let Err(e) = grpc.authentication.logout(LogoutRequest{}).await {
                                                    toaster.write().new_error(e.to_string());
                                                    return
                                                }
                                                *is_logged_in.write() = Login(None);
                                                show_menu.set(false);
                                            }
                                        })
                                    },
                                    flavor: ButtonFlavor::Danger,
                                    "Logout"
                                }
                            },
                            None => rsx! {
                                strong {
                                    "Login"
                                }
                                form {
                                    Field {
                                        label: "Username",
                                        TextInput{
                                            oninput: move |e: FormEvent| {
                                                login_form.write().username = e.value.clone();
                                            },
                                            value: TextInputType::Text(login_form.read().username.clone()),
                                        }
                                    }
                                    Field {
                                        label: "Password",
                                        TextInput{
                                            oninput: move |e: FormEvent| {
                                                login_form.write().password = e.value.clone();
                                            },
                                            value: TextInputType::Password(login_form.read().password.clone()),
                                        }
                                    }
                                    Button {
                                        onclick: move |_| {
                                            cx.spawn({
                                                to_owned!(toaster, login_form, grpc, is_logged_in, show_menu);
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

                                                    *is_logged_in.write() = Login(Some(claims));
                                                    show_menu.set(false);
                                                }
                                            })
                                        },
                                        flavor: ButtonFlavor::Success,
                                        "Login"
                                    },
                                }
                            },
                        }
                    }
                }
            }
        }
    })
}
