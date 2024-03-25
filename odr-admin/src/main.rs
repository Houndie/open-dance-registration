#![allow(non_snake_case)]
use common::proto::ClaimsRequest;
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use hooks::{
    login::{use_login, use_login_provider},
    toasts::{use_toasts, use_toasts_provider},
    use_grpc_client, use_grpc_client_provider,
};
use log::LevelFilter;
use pages::Routes;
use tonic::Code;

use crate::{components::with_toasts::WithToasts, hooks::login::Login};

pub mod components;
pub mod hooks;
pub mod pages;

fn main() {
    dioxus_logger::init(LevelFilter::Info).expect("failed to init logger");
    dioxus_web::launch(App);
}

fn App(cx: Scope) -> Element {
    use_toasts_provider(cx);
    use_grpc_client_provider(cx);
    use_login_provider(cx);
    use_check_login_state(cx);
    cx.render(rsx! {
        WithToasts {
            Router::<Routes> {}
        }
    })
}

fn use_check_login_state(cx: Scope) {
    let is_logged_in = use_login(cx).unwrap();
    let grpc = use_grpc_client(cx).unwrap();
    let toaster = use_toasts(cx).unwrap();

    use_future(cx, (), |_| {
        to_owned!(is_logged_in, grpc, toaster);
        async move {
            let claims = match grpc.authentication.claims(ClaimsRequest {}).await {
                Ok(response) => response.into_inner().claims,
                Err(e) => match e.code() {
                    Code::Unauthenticated => None,
                    _ => {
                        toaster.write().new_error(e.to_string());
                        return;
                    }
                },
            };

            *is_logged_in.write() = Login(claims);
        }
    });
}
