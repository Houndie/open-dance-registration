use crate::components::page::Page as GenericPage;
use dioxus::prelude::*;

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    log::info!("{}", id); // temporarily silencing warning
    cx.render(rsx! {
        GenericPage {
            title: "Modify Registration Schema".to_owned(),
            button {
                class: "btn btn-primary",
                "Add Registration Schema"
            }
        }
    })
}
