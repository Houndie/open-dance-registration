use crate::server_functions::ProtoWrapper;
use common::proto::{
    QueryEventsRequest, QueryEventsResponse, UpsertEventsRequest, UpsertEventsResponse,
};
use dioxus::prelude::*;

#[cfg(feature = "server")]
mod server_only {
    use crate::api::event::Service;
    use common::proto::{
        event_service_server::EventService, QueryEventsRequest, QueryEventsResponse,
        UpsertEventsRequest, UpsertEventsResponse,
    };
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
}

#[cfg(feature = "server")]
pub use server_only::AnyService;

#[server]
pub async fn upsert(
    request: ProtoWrapper<UpsertEventsRequest>,
) -> Result<ProtoWrapper<UpsertEventsResponse>, ServerFnError> {
    use crate::server_functions::status_to_server_fn_error;

    let service: AnyService = extract::<FromContext<AnyService>, _>().await?.0;
    service
        .upsert(tonic::Request::new(request.0))
        .await
        .map(|r| ProtoWrapper(r.into_inner()))
        .map_err(status_to_server_fn_error)
}

#[server]
pub async fn query(
    request: ProtoWrapper<QueryEventsRequest>,
) -> Result<ProtoWrapper<QueryEventsResponse>, ServerFnError> {
    use crate::server_functions::status_to_server_fn_error;

    let service: AnyService = extract::<FromContext<AnyService>, _>().await?.0;
    service
        .query(tonic::Request::new(request.0))
        .await
        .map(|r| ProtoWrapper(r.into_inner()))
        .map_err(status_to_server_fn_error)
}
