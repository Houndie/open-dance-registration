use common::proto::{
    event_service_client::EventServiceClient,
    organization_service_client::OrganizationServiceClient,
};
use dioxus::prelude::*;

pub mod toasts;

#[derive(Clone)]
pub struct GrpcContext {
    pub events: EventServiceClient<tonic_web_wasm_client::Client>,
    pub organizations: OrganizationServiceClient<tonic_web_wasm_client::Client>,
}

impl GrpcContext {
    fn new(base_url: String) -> Self {
        let web_client = tonic_web_wasm_client::Client::new(base_url);
        Self {
            events: EventServiceClient::new(web_client.clone()),
            organizations: OrganizationServiceClient::new(web_client),
        }
    }
}

pub fn use_grpc_client_provider(cx: &ScopeState) {
    use_context_provider(cx, || GrpcContext::new("http://localhost:50051".to_owned()));
}

pub fn use_grpc_client(cx: &ScopeState) -> Option<GrpcContext> {
    // Cloning grpc clients share the existing channels.
    // Cloning lets us perform multiple requests in parallel, as they all require &mut self for
    // tower reasons
    use_context::<GrpcContext>(cx).cloned()
}
