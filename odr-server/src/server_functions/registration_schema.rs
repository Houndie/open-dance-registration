use crate::server_functions::ProtoWrapper;
use common::proto::{
    QueryRegistrationSchemasRequest, QueryRegistrationSchemasResponse,
    UpsertRegistrationSchemasRequest, UpsertRegistrationSchemasResponse,
};
use dioxus::prelude::*;

#[cfg(feature = "server")]
mod server_only {
    use crate::api::registration_schema::Service;
    use common::proto::{
        registration_schema_service_server::RegistrationSchemaService,
        QueryRegistrationSchemasRequest, QueryRegistrationSchemasResponse,
        UpsertRegistrationSchemasRequest, UpsertRegistrationSchemasResponse,
    };
    use odr_core::store::registration_schema::SqliteStore;
    use std::sync::Arc;
    use tonic::{Request, Response, Status};

    #[derive(Clone)]
    pub enum AnyService {
        Sqlite(Arc<Service<SqliteStore>>),
    }

    impl AnyService {
        pub fn new_sqlite(store: Arc<Service<SqliteStore>>) -> Self {
            AnyService::Sqlite(store)
        }

        pub async fn upsert(
            &self,
            request: Request<UpsertRegistrationSchemasRequest>,
        ) -> Result<Response<UpsertRegistrationSchemasResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.upsert_registration_schemas(request).await,
            }
        }

        pub async fn query(
            &self,
            request: Request<QueryRegistrationSchemasRequest>,
        ) -> Result<Response<QueryRegistrationSchemasResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.query_registration_schemas(request).await,
            }
        }
    }
}

#[cfg(feature = "server")]
pub use server_only::AnyService;

#[server]
pub async fn upsert(
    request: ProtoWrapper<UpsertRegistrationSchemasRequest>,
) -> Result<ProtoWrapper<UpsertRegistrationSchemasResponse>, ServerFnError> {
    use crate::server_functions::status_to_server_fn_error;

    let service: AnyService = extract::<FromContext<AnyService>, _>().await?.0;
    service
        .upsert(tonic::Request::new(request.0))
        .await
        .map(|r| ProtoWrapper(r.into_inner()))
        .map_err(status_to_server_fn_error)
}

#[server]
pub async fn query(
    request: ProtoWrapper<QueryRegistrationSchemasRequest>,
) -> Result<ProtoWrapper<QueryRegistrationSchemasResponse>, ServerFnError> {
    use crate::server_functions::status_to_server_fn_error;

    let service: AnyService = extract::<FromContext<AnyService>, _>().await?.0;
    service
        .query(tonic::Request::new(request.0))
        .await
        .map(|r| ProtoWrapper(r.into_inner()))
        .map_err(status_to_server_fn_error)
}
