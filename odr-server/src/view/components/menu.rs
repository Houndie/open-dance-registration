use dioxus::prelude::*;

#[component]
pub fn Menu(title: ReadOnlySignal<String>, children: Element) -> Element {
    rsx! {
        h3 {
            class: "subtitle is-3",
            "{title}"
        }
        div {
            class: "menu",
            { children }
        }
    }
}
