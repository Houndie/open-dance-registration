use dioxus::prelude::*;

use crate::components::form::{Button, ButtonFlavor};

#[component]
pub fn Modal(
    onsubmit: EventHandler<()>,
    onclose: EventHandler<()>,
    title: ReadOnlySignal<String>,
    disable_submit: bool,
    children: Element,
    success_text: ReadOnlySignal<String>,
) -> Element {
    rsx! {
        div {
            class: "modal is-active",
            div {
                class: "modal-background",
                onclick: move |_| onclose.call(()),
            }
            div {
                class: "modal-card",
                header {
                    class: "modal-card-head",
                    p {
                        class: "modal-card-title",
                        "{title}"
                    }
                    button {
                        class: "delete",
                        "aria-label": "close",
                        onclick: move |_| onclose.call(()),
                    }
                }
                section {
                    class: "modal-card-body",
                    { children }
                }
                footer {
                    class: "modal-card-foot",
                    Button {
                        flavor: ButtonFlavor::Success,
                        disabled: disable_submit,
                        onclick: move |_| {
                            onsubmit(())
                        },
                        "{success_text}"
                    }
                    Button {
                        onclick: move |_| onclose(()),
                        "Cancel"
                    }
                }
            }
        }
    }
}
