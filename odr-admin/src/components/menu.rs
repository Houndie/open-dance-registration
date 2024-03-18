use dioxus::prelude::*;

pub mod event;
pub mod organization;
pub mod site;

#[component]
pub fn Menu<'a>(cx: Scope, title: &'a str, children: Element<'a>) -> Element<'a> {
    cx.render(rsx! {
        h3 {
            class: "subtitle is-3",
            "{title}"
        }
        div {
            class: "menu",
            &children
        }
    })
}
