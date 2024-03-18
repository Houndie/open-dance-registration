use dioxus::prelude::*;

use crate::{components::breadcrumb::Breadcrumb, pages::Routes};

#[component]
pub fn Page<'a>(
    cx: Scope,
    title: String,
    children: Element<'a>,
    style: Option<String>,
    breadcrumb: Option<Vec<(String, Option<Routes>)>>,
    menu: Option<Element<'a>>,
) -> Element {
    let style = match style {
        Some(style) => style.as_str(),
        None => "",
    };

    cx.render(rsx!(
        div {
            style: "{style}",
            if let Some(menu) = menu {
                rsx!{
                    div {
                        class: "has-background-grey-light",
                        style: "position: sticky; display: inline-block; vertical-align: top; overflow-y: auto; width: 400px; height: 100vh; padding: 10px",
                        menu
                    }
                }
            }
            div {
                style: "display: inline-block; padding: 20px;",
                h1 {
                    class: "title is-1",
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
