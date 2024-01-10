use dioxus::prelude::*;

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    log::info!("{}", id); // temporarily silencing warning
    cx.render(rsx! { div {
        class: "container",
        h2 {
            "Modify Registration Schema"
        }
        button {
            class: "btn btn-primary",
            "Add Registration Schema"
        }
    }})
}
