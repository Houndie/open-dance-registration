use crate::hooks::toasts::{use_toasts_provider, ToastManager};
use dioxus::prelude::*;

#[component]
pub fn WithToasts(initial_errors: Vec<String>, children: Element) -> Element {
    let mut toaster = use_signal(|| ToastManager::with_toasts(initial_errors));
    use_toasts_provider(toaster);
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
