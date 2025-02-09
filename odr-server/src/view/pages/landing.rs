use crate::{
    hooks::toasts::use_toasts,
    server_functions::{
        organization::{query, upsert},
        ProtoWrapper,
    },
    view::{
        app::Routes,
        components::{
            form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
            menu::Menu as GenericMenu,
            modal::Modal,
            page::Page as GenericPage,
            table::Table,
            with_toasts::WithToasts,
        },
    },
};
use common::proto::{Organization, QueryOrganizationsRequest, UpsertOrganizationsRequest};
use dioxus::prelude::*;

#[component]
pub fn Page() -> Element {
    let res = use_server_future(|| async {
        query(QueryOrganizationsRequest { query: None })
            .await
            .map(|r| ProtoWrapper(r))
    })?;

    let ProtoWrapper(res) = match res() {
        Some(Ok(res)) => res,
        Some(Err(e)) => {
            return rsx! {
                WithToasts{
                    initial_errors: vec![e.to_string()],
                }
            };
        }
        None => return rsx! {},
    };

    let menu = rsx! {
        Menu {
            highlight: MenuItem::Home,
        }
    };

    rsx! {
        GenericPage {
            title: "My Organizations".to_owned(),
            breadcrumb: vec![
                ("Home".to_owned(), None)
            ],
            menu: menu,
            PageBody{
                orgs: res.organizations,
            }
        }
    }
}

#[component]
fn PageBody(orgs: Vec<Organization>) -> Element {
    let nav = use_navigator();

    let mut orgs = use_signal(|| orgs);
    let mut show_org_modal = use_signal(|| false);

    let org_modal = if *show_org_modal.read() {
        rsx! {
            OrganizationModal {
                onsubmit: move |organization| {
                    show_org_modal.set(false);
                    orgs.write().push(organization);
                },
                onclose: move |_| show_org_modal.set(false),
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        Table {
            is_striped: true,
            is_fullwidth: true,
            thead {
                tr {
                    th {
                        class: "col-auto",
                        "Name"
                    }
                    th{
                        style: "width: 1px",
                    }
                }
            }
            tbody {
                { orgs.read().iter().map(|e|{
                    let id = e.id.clone();
                    rsx!{
                        tr {
                            key: "{e.id}",
                            td{
                                class: "col-auto",
                                "{e.name}"
                            }
                            td {
                                style: "width: 1px; white-space: nowrap;",
                                Button {
                                    flavor: ButtonFlavor::Info,
                                    onclick: move |_| {
                                        nav.push(Routes::OrganizationPage{
                                            org_id: id.clone(),
                                        });
                                    },
                                    "Edit Organization"
                                }
                            }
                        }
                    }
                }) }
            }
        }
        Button {
            flavor: ButtonFlavor::Info,
            onclick: move |_| show_org_modal.set(true),
            "Create New Organization"
        }
        { org_modal }
    }
}

#[derive(Clone, PartialEq)]
pub enum MenuItem {
    None,
    Home,
}

impl MenuItem {
    fn is_active(&self, this: &MenuItem) -> &'static str {
        if *self == *this {
            "is-active"
        } else {
            ""
        }
    }
}

#[component]
pub fn Menu(highlight: Option<MenuItem>) -> Element {
    let nav = use_navigator();
    let highlight = highlight.as_ref().cloned().unwrap_or(MenuItem::None);

    rsx! {
        GenericMenu {
            title: "ODR Admin",
            p {
                class: "menu-label",
                "General"
            }
            ul {
                class: "menu-list",
                li {
                    a {
                        class: highlight.is_active(&MenuItem::Home),
                        onclick: move |e| {
                            e.prevent_default();
                            nav.push(Routes::LandingPage);
                        },
                        "Home"
                    }
                }
            }
        }
    }
}

#[component]
fn OrganizationModal(onsubmit: EventHandler<Organization>, onclose: EventHandler<()>) -> Element {
    let mut organization_name = use_signal(String::new);
    let mut submitted = use_signal(|| false);
    let mut toaster = use_toasts();

    rsx! {
        Modal{
            onsubmit: move |_| {
                spawn({
                    submitted.set(true);
                    async move {
                        let rsp = upsert(UpsertOrganizationsRequest{
                            organizations: vec![Organization{
                                id: "".to_owned(),
                                name: organization_name.read().clone(),
                            }],
                        }).await;

                        let organization = match rsp {
                            Ok(mut rsp) => rsp.organizations.remove(0),
                            Err(e) => {
                                toaster.write().new_error(e.to_string());
                                return;
                            },
                        };

                        onsubmit.call(organization);
                    }
                });
            },
            onclose: move |_| onclose(()),
            disable_submit: *submitted.read(),
            title: "Create New Organization",
            success_text: "Create",
            form {
                div {
                    class: "mb-3",
                    Field {
                        label: "Organization Name",
                        TextInput {
                            value: TextInputType::Text(organization_name.read().clone()),
                            oninput: move |evt: FormEvent| organization_name.set(evt.value()),
                        }
                    }
                }
            }
        }
    }
}
