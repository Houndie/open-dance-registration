#[cfg(feature = "server")]
mod server_only {
    use crate::{
        api::permission::Service,
        proto::{
            permission_service_server::PermissionService, DeletePermissionsRequest,
            DeletePermissionsResponse, QueryPermissionsRequest, QueryPermissionsResponse,
            UpsertPermissionsRequest, UpsertPermissionsResponse,
        },
        server_functions::{tonic_request, tonic_response, Error},
        store::permission::SqliteStore,
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
            request: Request<UpsertPermissionsRequest>,
        ) -> Result<Response<UpsertPermissionsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.upsert_permissions(request).await,
            }
        }

        pub async fn query(
            &self,
            request: Request<QueryPermissionsRequest>,
        ) -> Result<Response<QueryPermissionsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.query_permissions(request).await,
            }
        }

        pub async fn delete(
            &self,
            request: Request<DeletePermissionsRequest>,
        ) -> Result<Response<DeletePermissionsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.delete_permissions(request).await,
            }
        }
    }

    pub async fn upsert(
        request: UpsertPermissionsRequest,
    ) -> Result<UpsertPermissionsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let response = service
            .upsert(tonic_request(request).await?)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(
        request: QueryPermissionsRequest,
    ) -> Result<QueryPermissionsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let response = service
            .query(tonic_request(request).await?)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn delete(
        request: DeletePermissionsRequest,
    ) -> Result<DeletePermissionsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let response = service
            .delete(tonic_request(request).await?)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }
}

#[cfg(feature = "server")]
pub use server_only::{delete, query, upsert, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::{
        proto::{
            permission_service_client::PermissionServiceClient, DeletePermissionsRequest,
            DeletePermissionsResponse, QueryPermissionsRequest, QueryPermissionsResponse,
            UpsertPermissionsRequest, UpsertPermissionsResponse,
        },
        server_functions::{wasm_client, Error},
    };

    pub async fn upsert(
        request: UpsertPermissionsRequest,
    ) -> Result<UpsertPermissionsResponse, Error> {
        let mut client = PermissionServiceClient::new(wasm_client());

        client
            .upsert_permissions(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(
        request: QueryPermissionsRequest,
    ) -> Result<QueryPermissionsResponse, Error> {
        let mut client = PermissionServiceClient::new(wasm_client());

        client
            .query_permissions(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn delete(
        request: DeletePermissionsRequest,
    ) -> Result<DeletePermissionsResponse, Error> {
        let mut client = PermissionServiceClient::new(wasm_client());

        client
            .delete_permissions(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{delete, query, upsert};
