use dioxus::prelude::*;

use crate::{
    hooks::toasts::use_toasts,
    server_functions::authentication::logout,
    view::{
        app::Routes,
        components::{breadcrumb::Breadcrumb, with_toasts::WithToasts},
    },
};

use common::proto::{Claims, LogoutRequest};

#[component]
pub fn Page(
    title: String,
    children: Element,
    style: Option<ReadOnlySignal<String>>,
    breadcrumb: Option<Vec<(String, Option<Routes>)>>,
    menu: Option<Element>,
    claims: ReadOnlySignal<Claims>,
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
        WithToasts{
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
                            UserMenu {
                                claims: claims,
                            }
                        }
                    }
                    { breadcrumb }
                    { children }
                }
            }
        }
    }
}

#[component]
fn UserMenu(claims: ReadOnlySignal<Claims>) -> Element {
    let mut show_menu = use_signal(|| false);
    let menu_is_active = if *show_menu.read() { "is-active" } else { "" };

    let nav = use_navigator();
    let mut toaster = use_toasts();

    rsx! {
        div {
            class: "dropdown {menu_is_active} is-right",
            div {
                class: "dropdown-trigger",
                button {
                    class: "button",
                    onclick: move |_| {
                        let new_show_menu = !(&*show_menu.read());
                        show_menu.set(new_show_menu);
                    },
                    span{
                        "Login/Signup"
                    }
                    span {
                        class: "icon is-small",
                        "⌄"
                    }
                }
            }
            div {
                class: "dropdown-menu",
                div {
                    class: "dropdown-content",
                    div {
                        class: "dropdown-item",
                        style: "min-width: 300px",
                        div {
                            class: "menu",
                            ul {
                                class: "menu-list",
                                li {
                                    a {
                                        onclick: move |e| {
                                            e.prevent_default();
                                            nav.push(Routes::ProfilePage);
                                        },
                                        "My Profile"
                                    }
                                }
                                li {
                                    a {
                                        onclick: move |e| {
                                            e.prevent_default();
                                            spawn(async move {
                                                if let Err(e) = logout(LogoutRequest{}).await {
                                                    toaster.write().new_error(e.to_string());
                                                    return
                                                }
                                                nav.push(Routes::LoginPage);
                                            });
                                        },
                                        "Logout"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
