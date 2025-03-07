#[cfg(feature = "server")]
mod server_only {
    use crate::{
        proto::{
            event_service_client::EventServiceClient, QueryEventsRequest, QueryEventsResponse,
            UpsertEventsRequest, UpsertEventsResponse,
        },
        server_functions::{tonic_request, tonic_response, Error, InternalServer},
    };
    use dioxus::prelude::*;

    pub async fn upsert(request: UpsertEventsRequest) -> Result<UpsertEventsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = EventServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .upsert_events(tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_response(response))
    }

    pub async fn query(request: QueryEventsRequest) -> Result<QueryEventsResponse, Error> {
        let server: InternalServer = extract::<FromContext<InternalServer>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let mut client = EventServiceClient::new(server);

        let tonic_request = tonic_request(request)?;

        let response = client
            .query_events(tonic_request)
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
            event_service_client::EventServiceClient, QueryEventsRequest, QueryEventsResponse,
            UpsertEventsRequest, UpsertEventsResponse,
        },
        server_functions::{wasm_client, Error},
    };

    pub async fn upsert(request: UpsertEventsRequest) -> Result<UpsertEventsResponse, Error> {
        let mut client = EventServiceClient::new(wasm_client());

        client
            .upsert_events(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(request: QueryEventsRequest) -> Result<QueryEventsResponse, Error> {
        let mut client = EventServiceClient::new(wasm_client());

        client
            .query_events(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }
}

#[cfg(feature = "web")]
pub use web_only::{query, upsert};
