#[cfg(feature = "server")]
mod server_only {
    use crate::{api::authentication::Service, server_functions::Error};
    use common::proto::{
        authentication_service_server::AuthenticationService, ClaimsRequest, ClaimsResponse,
        LoginRequest, LoginResponse, LogoutRequest, LogoutResponse,
    };
    use dioxus::prelude::*;
    use odr_core::store::{
        keys::SqliteStore as KeySqliteStore, user::SqliteStore as UserSqliteStore,
    };
    use std::sync::Arc;
    use tonic::{metadata::MetadataMap, Request, Response, Status};

    #[derive(Clone)]
    pub enum AnyService {
        Sqlite(Arc<Service<KeySqliteStore, UserSqliteStore>>),
    }

    impl AnyService {
        pub fn new_sqlite(store: Arc<Service<KeySqliteStore, UserSqliteStore>>) -> Self {
            AnyService::Sqlite(store)
        }

        pub async fn login(
            &self,
            request: Request<LoginRequest>,
        ) -> Result<Response<LoginResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.login(request).await,
            }
        }

        pub async fn logout(
            &self,
            request: Request<LogoutRequest>,
        ) -> Result<Response<LogoutResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.logout(request).await,
            }
        }

        pub async fn claims(
            &self,
            request: Request<ClaimsRequest>,
        ) -> Result<Response<ClaimsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.claims(request).await,
            }
        }
    }

    pub async fn login(request: LoginRequest) -> Result<LoginResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let server_context = server_context();

        let mut tonic_request = tonic::Request::new(request);
        *tonic_request.metadata_mut() =
            MetadataMap::from_headers(server_context.request_parts().headers.clone());

        let mut response = service.login(tonic_request).await.map_err(Error::GrpcError);

        if let Ok(ref mut response) = response {
            let metadata = std::mem::take(response.metadata_mut());
            server_context
                .response_parts_mut()
                .headers
                .extend(metadata.into_headers());
        }

        response.map(|r| r.into_inner())
    }

    pub async fn logout(request: LogoutRequest) -> Result<LogoutResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let server_context = server_context();

        let mut tonic_request = tonic::Request::new(request);
        *tonic_request.metadata_mut() =
            MetadataMap::from_headers(server_context.request_parts().headers.clone());

        let mut response = service
            .logout(tonic_request)
            .await
            .map_err(Error::GrpcError);

        if let Ok(ref mut response) = response {
            let metadata = std::mem::take(response.metadata_mut());
            server_context
                .response_parts_mut()
                .headers
                .extend(metadata.into_headers());
        }

        response.map(|r| r.into_inner())
    }

    pub async fn claims(request: ClaimsRequest) -> Result<ClaimsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let server_context = server_context();

        let mut tonic_request = tonic::Request::new(request);
        *tonic_request.metadata_mut() =
            MetadataMap::from_headers(server_context.request_parts().headers.clone());

        let mut response = service
            .claims(tonic_request)
            .await
            .map_err(Error::GrpcError);

        if let Ok(ref mut response) = response {
            let metadata = std::mem::take(response.metadata_mut());
            server_context
                .response_parts_mut()
                .headers
                .extend(metadata.into_headers());
        }

        response.map(|r| r.into_inner())
    }
}

#[cfg(feature = "server")]
pub use server_only::{claims, login, logout, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::server_functions::{wasm_client, Error};
    use common::proto::{
        authentication_service_client::AuthenticationServiceClient, ClaimsRequest, ClaimsResponse,
        LoginRequest, LoginResponse, LogoutRequest, LogoutResponse,
    };

    pub async fn login(request: LoginRequest) -> Result<LoginResponse, Error> {
        let mut client = AuthenticationServiceClient::new(wasm_client());

        client
            .login(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn logout(request: LogoutRequest) -> Result<LogoutResponse, Error> {
        let mut client = AuthenticationServiceClient::new(wasm_client());

        client
            .logout(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn claims(request: ClaimsRequest) -> Result<ClaimsResponse, Error> {
        let mut client = AuthenticationServiceClient::new(wasm_client());

        client
            .claims(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{claims, login, logout};
