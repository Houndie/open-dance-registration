use dioxus::prelude::*;

pub mod event;
pub mod organization;
pub mod profile;
pub mod site;

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
