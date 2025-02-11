#[cfg(feature = "server")]
mod server_only {
    use crate::{api::event::Service, server_functions::Error};
    use common::proto::{
        event_service_server::EventService, QueryEventsRequest, QueryEventsResponse,
        UpsertEventsRequest, UpsertEventsResponse,
    };
    use dioxus::prelude::*;
    use odr_core::store::event::SqliteStore;
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
            request: Request<UpsertEventsRequest>,
        ) -> Result<Response<UpsertEventsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.upsert_events(request).await,
            }
        }

        pub async fn query(
            &self,
            request: Request<QueryEventsRequest>,
        ) -> Result<Response<QueryEventsResponse>, Status> {
            match self {
                AnyService::Sqlite(service) => service.query_events(request).await,
            }
        }
    }

    pub async fn upsert(request: UpsertEventsRequest) -> Result<UpsertEventsResponse, Error> {
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        service
            .upsert(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError)
    }

    pub async fn query(request: QueryEventsRequest) -> Result<QueryEventsResponse, Error> {
        println!("in event");
        let service: AnyService = extract::<FromContext<AnyService>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let x = service
            .query(tonic::Request::new(request))
            .await
            .map(|r| r.into_inner())
            .map_err(Error::GrpcError);
        println!("out event");
        x
    }
}

#[cfg(feature = "server")]
pub use server_only::{query, upsert, AnyService};

#[cfg(feature = "web")]
mod web_only {
    use crate::server_functions::{wasm_client, Error};
    use common::proto::{
        event_service_client::EventServiceClient, QueryEventsRequest, QueryEventsResponse,
        UpsertEventsRequest, UpsertEventsResponse,
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
