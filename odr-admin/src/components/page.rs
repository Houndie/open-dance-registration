use dioxus::prelude::*;

use crate::hooks::toasts::use_toasts;

#[component]
pub fn Page<'a>(cx: Scope, title: String, children: Element<'a>) -> Element {
    let toast_manager = use_toasts(cx).unwrap();
    log::info!("rerender");

    cx.render(rsx!(
        div {
            class: "container",
            h2 {
                "{title}"
            }
            &children
            div {
                class: "toast-container",
                toast_manager.read().0.toasts().enumerate().map(|(idx, toast)| {
                    let toast_manager = toast_manager.clone();
                    cx.render(rsx!(
                        div {
                            key: "{idx}",
                            class: "toast",
                            role: "alert",
                            div {
                                class: "toast-header",
                                "{toast.title}",
                                button {
                                    "type": "button",
                                    class: "btn-close",
                                    onclick: move |_| {
                                        toast_manager.with_mut(|toast_manager| toast_manager.0.remove_toast(idx));
                                    },
                                }
                            }
                            div {
                                class: "toast-body",
                                "{toast.body}",
                            }
                        }
                    ))
                })
            }
        }
    ))
}
