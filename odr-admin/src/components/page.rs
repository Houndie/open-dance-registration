use dioxus::prelude::*;

use crate::{components::breadcrumb::Breadcrumb, pages::Routes};

#[component]
pub fn Page<'a>(
    cx: Scope,
    title: String,
    children: Element<'a>,
    style: Option<String>,
    breadcrumb: Option<Vec<(String, Option<Routes>)>>,
) -> Element {
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
                if let Some(breadcrumb) = breadcrumb {
                    rsx! {
                        Breadcrumb {
                            items: breadcrumb.clone(),
                        }
                    }
                }
                &children
            }
        }
    ))
}
