#[cfg(feature = "server")]
mod server_only {
    use crate::{
        api::user::Service,
        proto::{
            user_service_server::UserService, QueryUsersRequest, QueryUsersResponse,
            UpsertUsersRequest, UpsertUsersResponse,
        },
        server_functions::{tonic_request, tonic_response, Error},
        store::user::SqliteStore,
    };
    use dioxus::prelude::*;
    use std::sync::Arc;
    use tonic::{metadata::MetadataMap, Request, Response, Status};

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
            request: Request<UpsertUsersRequest>,
        ) -> Result<Response<UpsertUsersResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.upsert_users(request).await,
            }
        }

        pub async fn query(
            &self,
            request: Request<QueryUsersRequest>,
        ) -> Result<Response<QueryUsersResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.query_users(request).await,
            }
        }
    }

    pub async fn upsert(request: UpsertUsersRequest) -> Result<UpsertUsersResponse, Error> {
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

    pub async fn query(request: QueryUsersRequest) -> Result<QueryUsersResponse, Error> {
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
            user_service_client::UserServiceClient, QueryUsersRequest, QueryUsersResponse,
            UpsertUsersRequest, UpsertUsersResponse,
        },
        server_functions::{wasm_client, Error},
    };

    pub async fn upsert(request: UpsertUsersRequest) -> Result<UpsertUsersResponse, Error> {
        let mut client = UserServiceClient::new(wasm_client());

        client
            .upsert_users(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(request: QueryUsersRequest) -> Result<QueryUsersResponse, Error> {
        let mut client = UserServiceClient::new(wasm_client());

        client
            .query_users(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{query, upsert};
