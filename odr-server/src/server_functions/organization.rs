use crate::server_functions::ProtoWrapper;
use common::proto::{
    QueryOrganizationsRequest, QueryOrganizationsResponse, UpsertOrganizationsRequest,
    UpsertOrganizationsResponse,
};
use dioxus::prelude::*;

#[cfg(feature = "server")]
mod server_only {
    use crate::api::organization::Service;
    use common::proto::{
        organization_service_server::OrganizationService, QueryOrganizationsRequest,
        QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
    };
    use odr_core::store::organization::SqliteStore;
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
}

#[cfg(feature = "server")]
pub use server_only::AnyService;

#[server]
pub async fn upsert(
    request: ProtoWrapper<UpsertOrganizationsRequest>,
) -> Result<ProtoWrapper<UpsertOrganizationsResponse>, ServerFnError> {
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
    request: ProtoWrapper<QueryOrganizationsRequest>,
) -> Result<ProtoWrapper<QueryOrganizationsResponse>, ServerFnError> {
    use crate::server_functions::status_to_server_fn_error;

    let service: AnyService = extract::<FromContext<AnyService>, _>().await?.0;
    service
        .query(tonic::Request::new(request.0))
        .await
        .map(|r| ProtoWrapper(r.into_inner()))
        .map_err(status_to_server_fn_error)
}
