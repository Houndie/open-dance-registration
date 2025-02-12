use crate::{
    hooks::toasts::use_toasts,
    server_functions::authentication::{claims, login},
    view::{
        app::{Error, Routes},
        components::{
            form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
            with_toasts::WithToasts,
        },
    },
};
use common::proto::{ClaimsRequest, LoginRequest};
use dioxus::prelude::*;

#[derive(Default)]
struct LoginForm {
    username: String,
    password: String,
}

#[component]
pub fn Page() -> Element {
    let nav = use_navigator();
    let mut redirect_to = use_signal(|| None);
    use_effect(move || {
        if let Some(redirect) = redirect_to.read().clone() {
            nav.push(redirect);
        }
    });

    let results = use_server_future(|| async {
        claims(ClaimsRequest {})
            .await
            .map_err(Error::from_server_fn_error)?
            .claims
            .ok_or(Error::Unauthenticated)?;

        Ok(())
    })?;

    match results.suspend()?() {
        Ok(()) => {
            *redirect_to.write() = Some(Routes::LandingPage);
            return rsx! {};
        }
        Err(Error::Unauthenticated) => (),
        Err(e) => {
            return rsx! {
                WithToasts{
                    initial_errors: vec![e.to_string()],
                }
            };
        }
    };

    rsx! {
        WithToasts{
            PageBody {}
        }
    }
}

#[component]
fn PageBody() -> Element {
    let mut login_form = use_signal(LoginForm::default);
    let mut toaster = use_toasts();
    let nav = use_navigator();

    rsx! {
        div {
            class: "container",
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
                            async move {
                                let server_claims = match login(login_form.with(|login_form| {
                                    LoginRequest {
                                        email: login_form.username.clone(),
                                        password: login_form.password.clone(),
                                    }
                                })).await {
                                    Ok(response) => response.claims,
                                    Err(e) => {
                                        toaster.write().new_error(e.to_string());
                                        return
                                    },
                                };

                                if let None = server_claims {
                                    toaster.write().new_error("No Claims Found".to_string());
                                    return
                                }

                                nav.push(Routes::LandingPage);
                            }
                        });
                    },
                    flavor: ButtonFlavor::Success,
                    "Login"
                }
            }
        }
    }
}
