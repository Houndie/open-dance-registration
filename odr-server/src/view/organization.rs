use crate::server_functions::{organization::query, ProtoWrapper};
use common::proto::QueryOrganizationsRequest;
use dioxus::prelude::*;

#[component]
pub fn Page() -> Element {
    println!("HERE");
    let res = use_server_future(|| query(ProtoWrapper(QueryOrganizationsRequest { query: None })))?;

    let ProtoWrapper(res) = match res() {
        Some(Ok(res)) => res,
        Some(Err(e)) => return rsx! {"error {e}"},
        None => return rsx! {"loading..."},
    };

    rsx! {
        for org in res.organizations.iter() {
            div { "{org.name}" }
        }

        "hello worldd"
    }
}
