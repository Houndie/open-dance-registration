#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use log::LevelFilter;
use pages::Routes;

pub mod components;
pub mod hooks;
pub mod pages;

fn main() {
    dioxus_logger::init(LevelFilter::Info).expect("failed to init logger");
    dioxus_web::launch(App);
}

fn App(cx: Scope) -> Element {
    cx.render(rsx! {
        Router::<Routes> {}
    })
}
