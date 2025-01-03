use crate::{
    server_functions::{organization::query, ProtoWrapper},
    view::components::with_toasts::WithToasts,
};
use common::proto::QueryOrganizationsRequest;
use dioxus::prelude::*;

#[component]
pub fn Page() -> Element {
    let res = use_server_future(|| query(ProtoWrapper(QueryOrganizationsRequest { query: None })))?;

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

    let orgs = use_signal(|| res.organizations);

    rsx! {
        for org in orgs.iter() {
            div { "{org.name}" }
        }

        "hello worldd"
    }
}
