#[cfg(feature = "server")]
mod server_only {
    use crate::{
        proto::{
            organization_service_client::OrganizationServiceClient, QueryOrganizationsRequest,
            QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
        },
        server_functions::{tonic_request, tonic_response, Error, InternalServer},
    };
    use dioxus::prelude::*;

    pub async fn upsert(
        request: UpsertOrganizationsRequest,
    ) -> Result<UpsertOrganizationsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut organization_client = OrganizationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = organization_client
            .upsert_organizations(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(
        request: QueryOrganizationsRequest,
    ) -> Result<QueryOrganizationsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut organization_client = OrganizationServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = organization_client
            .query_organizations(tonic_request)
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
