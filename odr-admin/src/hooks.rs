use common::proto::{
    authorization_service_client::AuthorizationServiceClient,
    event_service_client::EventServiceClient,
    organization_service_client::OrganizationServiceClient,
    registration_schema_service_client::RegistrationSchemaServiceClient,
    registration_service_client::RegistrationServiceClient,
};
use dioxus::prelude::*;
use tonic_web_wasm_client::options::{Credentials, FetchOptions};

pub mod login;
pub mod toasts;

#[derive(Clone)]
pub struct GrpcContext {
    pub events: EventServiceClient<tonic_web_wasm_client::Client>,
    pub organizations: OrganizationServiceClient<tonic_web_wasm_client::Client>,
    pub registration_schema: RegistrationSchemaServiceClient<tonic_web_wasm_client::Client>,
    pub registration: RegistrationServiceClient<tonic_web_wasm_client::Client>,
    pub authorization: AuthorizationServiceClient<tonic_web_wasm_client::Client>,
}

impl GrpcContext {
    fn new(base_url: String) -> Self {
        let web_client = tonic_web_wasm_client::Client::new_with_options(
            base_url,
            FetchOptions::new().credentials(Credentials::Include),
        );
        Self {
            events: EventServiceClient::new(web_client.clone()),
            organizations: OrganizationServiceClient::new(web_client.clone()),
            registration_schema: RegistrationSchemaServiceClient::new(web_client.clone()),
            registration: RegistrationServiceClient::new(web_client.clone()),
            authorization: AuthorizationServiceClient::new(web_client),
        }
    }
}

pub fn use_grpc_client_provider(cx: &ScopeState) {
    use_context_provider(cx, || GrpcContext::new("http://localhost:50051".to_owned()));
}

pub fn use_grpc_client(cx: &ScopeState) -> Option<&GrpcContext> {
    use_context::<GrpcContext>(cx)
}
