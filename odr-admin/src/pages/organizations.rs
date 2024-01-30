use common::proto::{Organization, QueryOrganizationsRequest, UpsertOrganizationsRequest};
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;

use crate::{
    components::{modal::Modal, page::Page as GenericPage},
    hooks::{toasts::use_toasts, use_grpc_client, use_grpc_client_provider, OrganizationsClient},
    pages::Routes,
};

pub fn Page(cx: Scope) -> Element {
    use_grpc_client_provider::<OrganizationsClient>(cx);

    let orgs_client = use_grpc_client::<OrganizationsClient>(cx).unwrap();

    let toast_manager = use_toasts(cx).unwrap();

    let orgs = use_ref(cx, || Vec::new());

    let rsp: &UseFuture<Result<(), anyhow::Error>> = use_future(cx, (), |_| {
        to_owned!(orgs_client, orgs);
        async move {
            let response = orgs_client
                .lock()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .query_organizations(tonic::Request::new(QueryOrganizationsRequest {
                    query: None,
                }))
                .await
                .map_err(|e| anyhow::Error::new(e))?;

            orgs.with_mut(|orgs| *orgs = response.into_inner().organizations);
            Ok(())
        }
    });

    if let Some(err) = rsp.value().map(|rsp| rsp.as_ref().err()).flatten() {
        toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(err.to_string()));
    };

    let show_org_modal = use_state(cx, || false);

    let nav = use_navigator(cx);

    cx.render(rsx! {
        GenericPage {
            title: "My Organizations".to_owned(),
            table {
                class: "table table-striped",
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
                                    button {
                                        class: "btn btn-primary",
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
            button {
                class: "btn btn-primary",
                onclick: move |_| show_org_modal.set(true),
                "Create New Organization"
            }
        }
        if **show_org_modal { rsx!(OrganizationModal {
            do_submit: |organization| {
                show_org_modal.set(false);
                orgs.with_mut(|organizations| organizations.push(organization));
            },
            do_close: || show_org_modal.set(false),
        }) }
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
    let client = use_grpc_client::<OrganizationsClient>(cx).unwrap();
    let toast_manager = use_toasts(cx).unwrap();

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
                    to_owned!(client, organization_name, created, toast_manager);
                    async move {
                        let rsp = {
                            let lock = client.lock();
                            match lock {
                                Ok(mut unlocked) => {
                                    let rsp = unlocked.upsert_organizations(UpsertOrganizationsRequest{
                                        organizations: vec![Organization{
                                            id: "".to_owned(),
                                            name: organization_name.current().as_ref().clone(),
                                        }],
                                    }).await;

                                    match rsp {
                                        Ok(rsp) => Some(rsp),
                                        Err(e) => {
                                            toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(e.to_string()));
                                            None
                                        },
                                    }
                                },
                                Err(e) =>  {
                                    toast_manager.with_mut(|toast_manager| toast_manager.0.new_error(e.to_string()));
                                    None
                                },
                            }

                        };
                        if let Some(rsp) = rsp {
                            created.set(Some(rsp.into_inner().organizations.remove(0)));
                        };
                    }
                })
            },
            do_close: || do_close(),
            disable_submit: **submitted,
            title: "Create new Organization",
            form {
                div {
                    class: "mb-3",
                    label {
                        "for": "create-organization-name-input",
                        class: "form-label",
                        "Organization Name"
                    }
                    input {
                        id: "create-organization-name-input",
                        class: "form-control",
                        value: "{organization_name}",
                        oninput: move |evt| organization_name.set(evt.value.clone()),
                    }
                }
            }
        }
    })
}
