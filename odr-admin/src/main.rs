#![allow(non_snake_case)]
use common::proto::ClaimsRequest;
use dioxus::prelude::*;
use hooks::{
    login::{use_login, use_login_provider},
    toasts::{use_toasts, use_toasts_provider},
    use_grpc_client, use_grpc_client_provider,
};
use log::LevelFilter;
use pages::Routes;
use tonic::Code;

use crate::{components::with_toasts::WithToasts, hooks::login::LoginState};

pub mod components;
pub mod hooks;
pub mod pages;

fn main() {
    dioxus_logger::init(LevelFilter::Info).expect("failed to init logger");
    launch(App);
}

fn App() -> Element {
    use_toasts_provider();
    use_grpc_client_provider();
    use_login_provider();

    use_check_login_state();
    rsx! {
        WithToasts {
            Router::<Routes> {}
        }
    }
}

fn use_check_login_state() {
    let mut is_logged_in = use_login();
    let grpc = use_grpc_client();
    let mut toaster = use_toasts();

    let _ = use_resource(move || {
        let mut grpc = grpc.clone();
        async move {
            let res = grpc.authentication.claims(ClaimsRequest {}).await;
            match res {
                Ok(response) => {
                    *is_logged_in.write() =
                        LoginState::LoggedIn(response.into_inner().claims.unwrap())
                }
                Err(e) => match e.code() {
                    Code::Unauthenticated => *is_logged_in.write() = LoginState::LoggedOut,
                    _ => {
                        toaster.write().new_error(e.to_string());
                        return;
                    }
                },
            };
        }
    });
}
