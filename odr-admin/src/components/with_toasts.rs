use dioxus::prelude::*;

use crate::hooks::toasts::use_toasts;

#[component]
pub fn WithToasts(children: Element) -> Element {
    let mut toaster = use_toasts();
    rsx! {
        { children },
        { toaster.read().toasts().enumerate().map(|(idx, toast)| {
            let offset = idx * 9 + 2;
            rsx!{
                div {
                    key: "{idx}",
                    class: "notification is-warning",
                    style: "position: fixed; bottom: {offset}rem; right: 1.5rem; z-index: 1000; height: 7rem;",
                    role: "alert",
                    button {
                        class: "delete",
                        onclick: move |_| {
                            toaster.write().remove_toast(idx);
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
            }
        })}

    }
}
