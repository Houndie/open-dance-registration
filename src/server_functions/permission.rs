#[cfg(feature = "server")]
mod server_only {
    use crate::{
        proto::{
            permission_service_client::PermissionServiceClient, DeletePermissionsRequest,
            DeletePermissionsResponse, QueryPermissionsRequest, QueryPermissionsResponse,
            UpsertPermissionsRequest, UpsertPermissionsResponse,
        },
        server_functions::{tonic_request, tonic_response, Error, InternalServer},
    };
    use dioxus::prelude::*;

    pub async fn upsert(
        request: UpsertPermissionsRequest,
    ) -> Result<UpsertPermissionsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = PermissionServiceClient::new(server.clone());

        let tonic_request = tonic_request(request)?;

        let response = client
            .upsert_permissions(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(
        request: QueryPermissionsRequest,
    ) -> Result<QueryPermissionsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = PermissionServiceClient::new(server.clone());

        let tonic_request = tonic_request(request)?;

        let response = client
            .query_permissions(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn delete(
        request: DeletePermissionsRequest,
    ) -> Result<DeletePermissionsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = PermissionServiceClient::new(server.clone());

        let tonic_request = tonic_request(request)?;

        let response = client
            .delete_permissions(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }
}

#[cfg(feature = "server")]
pub use server_only::{delete, query, upsert};

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
