#![allow(non_snake_case)]
use dioxus::prelude::*;

fn main() {
    dioxus_web::launch(App);
}

fn App(cx: Scope) -> Element {
    cx.render(rsx! {
        div {
            "My Events"
        }

        table {
            tr {
                th {
                    "Name"
                }

                th {}
            }
        }

    })
}
