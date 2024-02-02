use dioxus::prelude::*;

pub fn Page(cx: Scope) -> Element {
    cx.render(rsx! {
        h1 { "Not Found" }
    })
}
