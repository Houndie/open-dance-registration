#[cfg(feature = "server")]
mod server_only {
    use crate::{
        proto::{
            authentication_service_client::AuthenticationServiceClient, ClaimsRequest,
            ClaimsResponse, LoginRequest, LoginResponse, LogoutRequest, LogoutResponse,
        },
        server_functions::{tonic_request, tonic_response, Error, InternalServer},
    };
    use dioxus::prelude::*;

    pub async fn login(request: LoginRequest) -> Result<LoginResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = AuthenticationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .login(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn logout(request: LogoutRequest) -> Result<LogoutResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = AuthenticationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .logout(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn claims(request: ClaimsRequest) -> Result<ClaimsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = AuthenticationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .claims(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }
}

#[cfg(feature = "server")]
pub use server_only::{claims, login, logout};

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
