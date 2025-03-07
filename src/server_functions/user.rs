#[cfg(feature = "server")]
mod server_only {
    use crate::{
        proto::{
            user_service_client::UserServiceClient, QueryUsersRequest, QueryUsersResponse,
            UpsertUsersRequest, UpsertUsersResponse,
        },
        server_functions::{tonic_request, tonic_response, Error, InternalServer},
    };
    use dioxus::prelude::*;

    pub async fn upsert(request: UpsertUsersRequest) -> Result<UpsertUsersResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut user_client = UserServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = user_client
            .upsert_users(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(request: QueryUsersRequest) -> Result<QueryUsersResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut user_client = UserServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = user_client
            .query_users(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }
}

#[cfg(feature = "server")]
pub use server_only::{query, upsert};

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
