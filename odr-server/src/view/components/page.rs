use dioxus::prelude::*;

use crate::view::{app::Routes, components::breadcrumb::Breadcrumb};

#[component]
pub fn Page(
    title: String,
    children: Element,
    style: Option<ReadOnlySignal<String>>,
    breadcrumb: Option<Vec<(String, Option<Routes>)>>,
    menu: Option<Element>,
) -> Element {
    let style = use_memo(move || style.map(|style| style.read().clone()).unwrap_or_default());

    let menu = menu.map(|menu| {
        rsx!{
            div {
                class: "has-background-grey-light",
                style: "position: sticky; display: inline-block; vertical-align: top; overflow-y: auto; width: 400px; height: 100vh; padding: 10px",
                { menu }
            }
        }});

    let breadcrumb = breadcrumb.map(|breadcrumb| {
        rsx! {
            Breadcrumb {
                items: breadcrumb.clone(),
            }
        }
    });

    rsx! {
        div {
            style: "{style}",
            { menu }
            div {
                style: "display: inline-block; padding: 20px; width: calc(100% - 400px);",
                div {
                    class: "columns",
                    div {
                        class: "column",
                        h1 {
                            class: "title is-1",
                            "{title}"
                        }
                    }
                    div {
                        class: "column is-one-third has-text-right",
                        //LoginMenu {}
                    }
                }
                { breadcrumb }
                { children }
            }
        }
    }
}
