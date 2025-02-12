#[cfg(feature = "server")]
mod server_only {
    use crate::{
        api::registration_schema::Service, server_functions::Error,
        store::registration_schema::SqliteStore,
    };
    use common::proto::{
        registration_schema_service_server::RegistrationSchemaService,
        QueryRegistrationSchemasRequest, QueryRegistrationSchemasResponse,
        UpsertRegistrationSchemasRequest, UpsertRegistrationSchemasResponse,
    };
    use dioxus::prelude::*;
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

    pub async fn upsert(
        request: UpsertRegistrationSchemasRequest,
    ) -> Result<UpsertRegistrationSchemasResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;
        service
            .upsert(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(
        request: QueryRegistrationSchemasRequest,
    ) -> Result<QueryRegistrationSchemasResponse, Error> {
        println!("in schema");
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;
        let x = service
            .query(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError);
        println!("out schema");
        x
    }
}

#[cfg(feature = "server")]
pub use server_only::{query, upsert, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::server_functions::{wasm_client, Error};
    use common::proto::{
        registration_schema_service_client::RegistrationSchemaServiceClient,
        QueryRegistrationSchemasRequest, QueryRegistrationSchemasResponse,
        UpsertRegistrationSchemasRequest, UpsertRegistrationSchemasResponse,
    };

    pub async fn upsert(
        request: UpsertRegistrationSchemasRequest,
    ) -> Result<UpsertRegistrationSchemasResponse, Error> {
        let mut client = RegistrationSchemaServiceClient::new(wasm_client());

        client
            .upsert_registration_schemas(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(
        request: QueryRegistrationSchemasRequest,
    ) -> Result<QueryRegistrationSchemasResponse, Error> {
        let mut client = RegistrationSchemaServiceClient::new(wasm_client());

        client
            .query_registration_schemas(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{query, upsert};
