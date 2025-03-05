#[cfg(feature = "server")]
mod server_only {
    use crate::{
        api::organization::Service,
        proto::{
            organization_service_server::OrganizationService, QueryOrganizationsRequest,
            QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
        },
        server_functions::{tonic_request, tonic_response, Error},
        store::organization::SqliteStore,
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
            request: Request<UpsertOrganizationsRequest>,
        ) -> Result<Response<UpsertOrganizationsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.upsert_organizations(request).await,
            }
        }

        pub async fn query(
            &self,
            request: Request<QueryOrganizationsRequest>,
        ) -> Result<Response<QueryOrganizationsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.query_organizations(request).await,
            }
        }
    }

    pub async fn upsert(
        request: UpsertOrganizationsRequest,
    ) -> Result<UpsertOrganizationsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let tonic_request = tonic_request(request).await?;

        let response = service
            .upsert(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(
        request: QueryOrganizationsRequest,
    ) -> Result<QueryOrganizationsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let tonic_request = tonic_request(request).await?;

        let response = service
            .query(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }
}

#[cfg(feature = "server")]
pub use server_only::{query, upsert, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::{
        proto::{
            organization_service_client::OrganizationServiceClient, QueryOrganizationsRequest,
            QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
        },
        server_functions::{wasm_client, Error},
    };

    pub async fn upsert(
        request: UpsertOrganizationsRequest,
    ) -> Result<UpsertOrganizationsResponse, Error> {
        let mut client = OrganizationServiceClient::new(wasm_client());

        client
            .upsert_organizations(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(
        request: QueryOrganizationsRequest,
    ) -> Result<QueryOrganizationsResponse, Error> {
        let mut client = OrganizationServiceClient::new(wasm_client());

        client
            .query_organizations(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{query, upsert};
