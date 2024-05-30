use common::proto::{Organization, QueryOrganizationsRequest, UpsertOrganizationsRequest};
use dioxus::prelude::*;

use crate::{
    components::{
        form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
        menu::site::{Menu, MenuItem},
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

pub fn Page() -> Element {
    log::info!("rendering organizations page");
    let grpc_client = use_grpc_client();

    let mut toaster = use_toasts();

    let mut orgs = use_signal(|| Vec::new());

    let orgs_success = use_resource(move || {
        let mut grpc_client = grpc_client.clone();
        async move {
            let result = grpc_client
                .organizations
                .query_organizations(tonic::Request::new(QueryOrganizationsRequest {
                    query: None,
                }))
                .await;

            let response = match result {
                Ok(rsp) => rsp,
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    return false;
                }
            };

            *orgs.write() = response.into_inner().organizations;
            true
        }
    });

    let mut show_org_modal = use_signal(|| false);

    let nav = use_navigator();

    let menu = rsx! {
        Menu {
            highlight: MenuItem::Home,
        }
    };

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
        None
    };

    let page_body = match &*orgs_success.value().read() {
        Some(true) => {
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
                                                nav.push(Routes::EventsPage{
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
        _ => None,
    };

    rsx! {
        GenericPage {
            title: "My Organizations".to_owned(),
            breadcrumb: vec![
                ("Home".to_owned(), None)
            ],
            menu: menu,
            { page_body }
        }
    }
}

#[component]
fn OrganizationModal(onsubmit: EventHandler<Organization>, onclose: EventHandler<()>) -> Element {
    let mut organization_name = use_signal(String::new);
    let mut submitted = use_signal(|| false);
    let client = use_grpc_client();
    let mut toaster = use_toasts();

    rsx! {
        Modal{
            onsubmit: move |_| {
                let mut client = client.clone();
                spawn({
                    submitted.set(true);
                    async move {
                        let rsp = client.organizations.upsert_organizations(UpsertOrganizationsRequest{
                            organizations: vec![Organization{
                                id: "".to_owned(),
                                name: organization_name.read().clone(),
                            }],
                        }).await;

                        let organization = match rsp {
                            Ok(rsp) => rsp.into_inner().organizations.remove(0),
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
