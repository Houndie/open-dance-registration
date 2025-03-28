#[cfg(feature = "server")]
mod server_only {
    use crate::{
        proto::{
            registration_service_client::RegistrationServiceClient, QueryRegistrationsRequest,
            QueryRegistrationsResponse, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
        },
        server_functions::{tonic_request, tonic_response, Error, InternalServer},
    };
    use dioxus::prelude::*;

    pub async fn upsert(
        request: UpsertRegistrationsRequest,
    ) -> Result<UpsertRegistrationsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = RegistrationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .upsert_registrations(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(
        request: QueryRegistrationsRequest,
    ) -> Result<QueryRegistrationsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = RegistrationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .query_registrations(tonic_request)
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
            registration_service_client::RegistrationServiceClient, QueryRegistrationsRequest,
            QueryRegistrationsResponse, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
        },
        server_functions::{wasm_client, Error},
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
