use dioxus::prelude::*;

#[component]
pub fn Page<'a>(cx: Scope, title: String, children: Element<'a>, style: Option<String>) -> Element {
    let style = match style {
        Some(style) => style.as_str(),
        None => "",
    };

    cx.render(rsx!(
        div {
            style: "{style}",
            div {
                class: "container",
                h1 {
                    class: "title",
                    "{title}"
                }
                &children
            }
        }
    ))
}
