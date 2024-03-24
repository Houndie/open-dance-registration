use dioxus::prelude::*;
use gloo_net::http::Method;
use web_sys::RequestCredentials;

use crate::{
    components::form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
    hooks::{login::use_login, toasts::use_toasts},
};

use common::rest::{LoginRequest, LoginResponse};

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
    let login_info = use_login(cx).unwrap();

    let toaster = use_toasts(cx).unwrap();
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
                                        to_owned!(login_info, toaster, login_form);
                                        async move {
                                            let body = login_form.with(|login_form| LoginRequest::Credentials {
                                                email: login_form.username.clone(),
                                                password: login_form.password.clone(),
                                            });

                                            let request = match gloo_net::http::RequestBuilder::new("http://localhost:3000/login").credentials(RequestCredentials::Include).method(Method::POST).json(&body) {
                                                Ok(request) => request,
                                                Err(e) => {
                                                    toaster.write().new_error(e.to_string());
                                                    return;
                                                },
                                            };

                                            let response = match request.send().await {
                                                Ok(response) => response,
                                                Err(e) => {
                                                    toaster.write().new_error(e.to_string());
                                                    return;
                                                },
                                            };

                                            let login_response = match response.json::<LoginResponse>().await {
                                                Ok(response) => response,
                                                Err(e) => {
                                                    toaster.write().new_error(e.to_string());
                                                    return;
                                                },
                                            };

                                            log::info!("{}", login_response.token);

                                            login_info.write().0 = Some(login_response.token);
                                        }
                                    })
                                },
                                flavor: ButtonFlavor::Success,
                                "Login"
                            },
                        }
                    }
                }
            }
        }
    })
}
