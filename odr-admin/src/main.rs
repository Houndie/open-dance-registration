#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use hooks::{login::use_login_provider, toasts::use_toasts_provider, use_grpc_client_provider};
use log::LevelFilter;
use pages::Routes;

use crate::components::with_toasts::WithToasts;

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
    cx.render(rsx! {
        WithToasts {
            Router::<Routes> {}
        }
    })
}
