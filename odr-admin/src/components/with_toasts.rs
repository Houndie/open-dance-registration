use dioxus::prelude::*;

use crate::hooks::toasts::{use_toasts, use_toasts_provider};

#[component]
pub fn WithToasts<'a>(cx: Scope, children: Element<'a>) -> Element {
    use_toasts_provider(cx);
    let toast_manager = use_toasts(cx).unwrap();
    cx.render(rsx!{
        &children,
        toast_manager.read().0.toasts().enumerate().map(|(idx, toast)| {
            let toast_manager = toast_manager.clone();
            let offset = idx * 9 + 2;
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

    })
}
