use dioxus::prelude::*;

#[component]
pub fn Page<'a>(cx: Scope, title: String, children: Element<'a>) -> Element {
    cx.render(rsx!(
        div {
            class: "container",
            h2 {
                "{title}"
            }
            &children
        }
    ))
}
