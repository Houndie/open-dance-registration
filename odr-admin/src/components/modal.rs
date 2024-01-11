use dioxus::prelude::*;

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
            style: "position: fixed; z-index: 1; padding-top: 100px; left: 0; top: 0; width: 100%; height: 100%; overflow: auto; background-color: rgb(0,0,0); background-color: rgba(0,0,0,0.4);",
            div {
                style: "margin: auto; width: 80%",
                div {
                    class: "card",
                    div {
                        div {
                            class: "card-header",
                            div {
                                class: "row",
                                div {
                                    class: "col",
                                    h5 {
                                        class: "card-title",
                                        "{title}"
                                    }
                                }
                                div {
                                    class: "col-1 d-flex justify-content-end",
                                    button {
                                        class: "btn-close",
                                        onclick: |_| do_close(),
                                    }
                                }
                            }
                        }
                        div {
                            class: "card-body",
                            &children,

                            div {
                                class: "d-flex flex-row-reverse",
                                div {
                                    class: "p-1 flex-shrink-1",
                                    button {
                                        class: "btn btn-primary",
                                        disabled: *disable_submit,
                                        onclick: |_| {
                                            log::info!("HI");
                                            do_submit()
                                        },
                                        "Create"
                                    }
                                }
                                div {
                                    class: "p-1 flex-shrink-1",
                                    button {
                                        class: "btn btn-secondary",
                                        onclick: |_| do_close(),
                                        "Cancel"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}
