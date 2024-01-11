use dioxus::prelude::*;

use crate::hooks::toasts::{use_toasts, use_toasts_provider};

#[component]
pub fn Page<'a>(cx: Scope, title: String, children: Element<'a>) -> Element {
    use_toasts_provider(cx);
    let toast_manager = use_toasts(cx).unwrap();

    cx.render(rsx!(
        div {
            class: "container",
            h2 {
                "{title}"
            }
            &children
            div {
                class: "toast-container",
                toast_manager.borrow().toasts().enumerate().map(|(idx, toast)| {
                    let toast_manager = toast_manager.clone();
                    cx.render(rsx!(
                        div {
                            key: "{idx}",
                            class: "toast",
                            div {
                                class: "toast-header",
                                "{toast.title}",
                                button {
                                    "type": "button",
                                    class: "btn-close",
                                    onclick: move |_| {
                                        toast_manager.borrow_mut().remove_toast(idx)
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
