#[cfg(feature = "server")]
mod server_only {
    use crate::{api::registration::Service, server_functions::Error};
    use common::proto::{
        registration_service_server::RegistrationService, QueryRegistrationsRequest,
        QueryRegistrationsResponse, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
    };
    use dioxus::prelude::*;
    use odr_core::store::registration::SqliteStore;
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
            request: Request<UpsertRegistrationsRequest>,
        ) -> Result<Response<UpsertRegistrationsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.upsert_registrations(request).await,
            }
        }

        pub async fn query(
            &self,
            request: Request<QueryRegistrationsRequest>,
        ) -> Result<Response<QueryRegistrationsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.query_registrations(request).await,
            }
        }
    }

    pub async fn upsert(
        request: UpsertRegistrationsRequest,
    ) -> Result<UpsertRegistrationsResponse, Error> {
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
        request: QueryRegistrationsRequest,
    ) -> Result<QueryRegistrationsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        service
            .query(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "server")]
pub use server_only::{query, upsert, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::server_functions::{wasm_client, Error};
    use common::proto::{
        registration_service_client::RegistrationServiceClient, QueryRegistrationsRequest,
        QueryRegistrationsResponse, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
    };

    pub async fn upsert(
        request: UpsertRegistrationsRequest,
    ) -> Result<UpsertRegistrationsResponse, Error> {
        let mut client = RegistrationServiceClient::new(wasm_client());

        client
            .upsert_registrations(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(
        request: QueryRegistrationsRequest,
    ) -> Result<QueryRegistrationsResponse, Error> {
        let mut client = RegistrationServiceClient::new(wasm_client());

        client
            .query_registrations(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{query, upsert};
