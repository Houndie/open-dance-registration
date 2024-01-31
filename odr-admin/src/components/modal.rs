use dioxus::prelude::*;

use crate::components::form::{Button, ButtonFlavor};

#[component]
pub fn Modal<'a, DoSubmit: Fn() -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    do_submit: DoSubmit,
    do_close: DoClose,
    title: &'a str,
    disable_submit: bool,
    children: Element<'a>,
) -> Element {
    cx.render(rsx! {
        div {
            class: "modal is-active",
            div {
                class: "modal-background",
                onclick: |_| do_close(),
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
                        onclick: |_| do_close(),
                    }
                }
                section {
                    class: "modal-card-body",
                    &children,
                }
                footer {
                    class: "modal-card-foot",
                    Button {
                        flavor: ButtonFlavor::Success,
                        disabled: *disable_submit,
                        onclick: |_| {
                            do_submit()
                        },
                        "Create"
                    }
                    Button {
                        onclick: |_| do_close(),
                        "Cancel"
                    }
                }
            }
        }
    })
}
