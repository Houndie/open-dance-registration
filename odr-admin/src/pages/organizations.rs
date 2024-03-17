use common::proto::{Organization, QueryOrganizationsRequest, UpsertOrganizationsRequest};
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;

use crate::{
    components::{
        form::{Button, ButtonFlavor, Field, TextInput, TextInputType},
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

pub fn Page(cx: Scope) -> Element {
    let grpc_client = use_grpc_client(cx).unwrap();

    let toaster = use_toasts(cx).unwrap();

    let orgs = use_ref(cx, || Vec::new());

    let orgs_success: &UseFuture<bool> = use_future(cx, (), |_| {
        to_owned!(grpc_client, orgs, toaster);
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

    let show_org_modal = use_state(cx, || false);

    let nav = use_navigator(cx);

    cx.render(rsx! {
        GenericPage {
            title: "My Organizations".to_owned(),
            breadcrumb: vec![
                ("Home".to_owned(), None)
            ],
            if matches!(orgs_success.value(), Some(true)) {
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
                            orgs.read().iter().map(|e|{
                                let id = e.id.clone();
                                rsx!{
                                    tr {
                                        key: "{e.id}",
                                        td{
                                            class: "col-auto",
                                            e.name.clone()
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
                            })
                        }
                    }
                    Button {
                        flavor: ButtonFlavor::Info,
                        onclick: move |_| show_org_modal.set(true),
                        "Create New Organization"
                    }
                    if **show_org_modal {
                        rsx!{
                            OrganizationModal {
                                do_submit: |organization| {
                                    show_org_modal.set(false);
                                    orgs.write().push(organization);
                                },
                                do_close: || show_org_modal.set(false),
                            }
                        }
                    }
                }
            }
        }
    })
}

#[component]
fn OrganizationModal<DoSubmit: Fn(Organization) -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let organization_name = use_state(cx, || "".to_owned());
    let submitted = use_state(cx, || false);
    let created = use_ref(cx, || None);
    let client = use_grpc_client(cx).unwrap();
    let toaster = use_toasts(cx).unwrap();

    {
        let mut created_mut = created.write_silent();
        if let Some(organization) = created_mut.as_mut() {
            created.needs_update();
            let organization = std::mem::take::<Organization>(organization);
            do_submit(organization);
        }
    }

    cx.render(rsx! {
        Modal{
            do_submit: move || {
                cx.spawn({
                    submitted.set(true);
                    to_owned!(client, organization_name, created, toaster);
                    async move {
                        let rsp = client.organizations.upsert_organizations(UpsertOrganizationsRequest{
                            organizations: vec![Organization{
                                id: "".to_owned(),
                                name: organization_name.current().as_ref().clone(),
                            }],
                        }).await;

                        match rsp {
                            Ok(rsp) => created.set(Some(rsp.into_inner().organizations.remove(0))),
                            Err(e) => {
                                toaster.write().new_error(e.to_string());
                            },
                        }
                    }
                })
            },
            do_close: || do_close(),
            disable_submit: **submitted,
            title: "Create New Organization",
            success_text: "Create",
            form {
                div {
                    class: "mb-3",
                    Field {
                        label: "Organization Name",
                        TextInput {
                            value: TextInputType::Text(organization_name.get().clone()),
                            oninput: move |evt: FormEvent| organization_name.set(evt.value.clone()),
                        }
                    }
                }
            }
        }
    })
}
