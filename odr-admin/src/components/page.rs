use dioxus::prelude::*;

use crate::hooks::toasts::use_toasts;

#[component]
pub fn Page<'a>(cx: Scope, title: String, children: Element<'a>) -> Element {
    let toast_manager = use_toasts(cx).unwrap();
    log::info!("rerender");

    cx.render(rsx!(
        div {
            class: "container",
            h1 {
                class: "title",
                "{title}"
            }
            &children
        }
        toast_manager.read().0.toasts().enumerate().map(|(idx, toast)| {
            let toast_manager = toast_manager.clone();
            let offset = idx * 9;
            cx.render(rsx!(
                div {
                    key: "{idx}",
                    class: "notification is-warning",
                    style: "position: fixed; bottom: {offset}rem; right: 1.5rem; z-index: 1000; height: 7rem;",
                    role: "alert",
                    button {
                        class: "delete",
                        onclick: move |_| {
                            toast_manager.with_mut(|toast_manager| toast_manager.0.remove_toast(idx));
                        },
                    }

                    h1 {
                        class: "title is-4",
                        "{toast.title}",
                    }
                    p {
                        "{toast.body}",
                    }
                }
            ))
        })
    ))
}
