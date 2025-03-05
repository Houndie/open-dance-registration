#[cfg(feature = "server")]
mod server_only {
    use crate::{
        api::authentication::Service,
        keys::StoreKeyManager,
        proto::{
            authentication_service_server::AuthenticationService, ClaimsRequest, ClaimsResponse,
            LoginRequest, LoginResponse, LogoutRequest, LogoutResponse,
        },
        server_functions::{tonic_response, tonic_unauthenticated_request, Error},
        store::{keys::SqliteStore as KeySqliteStore, user::SqliteStore as UserSqliteStore},
    };
    use dioxus::prelude::*;
    use std::sync::Arc;
    use tonic::{Request, Response, Status};

    #[derive(Clone)]
    pub enum AnyService {
        Sqlite(Arc<Service<StoreKeyManager<KeySqliteStore>, UserSqliteStore>>),
    }

    impl AnyService {
        pub fn new_sqlite(
            store: Arc<Service<StoreKeyManager<KeySqliteStore>, UserSqliteStore>>,
        ) -> Self {
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

        let tonic_request = tonic_unauthenticated_request(request)?;

        let response = service
            .login(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn logout(request: LogoutRequest) -> Result<LogoutResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let tonic_request = tonic_unauthenticated_request(request)?;

        let response = service
            .logout(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn claims(request: ClaimsRequest) -> Result<ClaimsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let tonic_request = tonic_unauthenticated_request(request)?;

        let response = service
            .claims(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }
}

#[cfg(feature = "server")]
pub use server_only::{claims, login, logout, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::{
        proto::{
            authentication_service_client::AuthenticationServiceClient, ClaimsRequest,
            ClaimsResponse, LoginRequest, LoginResponse, LogoutRequest, LogoutResponse,
        },
        server_functions::{wasm_client, Error},
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
