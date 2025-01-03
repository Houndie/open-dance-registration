use common::proto::{
    authentication_service_client::AuthenticationServiceClient,
    event_service_client::EventServiceClient,
    organization_service_client::OrganizationServiceClient,
    registration_schema_service_client::RegistrationSchemaServiceClient,
    registration_service_client::RegistrationServiceClient, user_service_client::UserServiceClient,
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
    pub authentication: AuthenticationServiceClient<tonic_web_wasm_client::Client>,
    pub user: UserServiceClient<tonic_web_wasm_client::Client>,
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
            authentication: AuthenticationServiceClient::new(web_client.clone()),
            user: UserServiceClient::new(web_client),
        }
    }
}

pub fn use_grpc_client_provider() {
    use_context_provider(|| GrpcContext::new("http://localhost:50051".to_owned()));
}

pub fn use_grpc_client() -> GrpcContext {
    use_context::<GrpcContext>()
}
