use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::{
    proto::{
        self, DeleteEventsResponse, ListEventsRequest, ListEventsResponse, UpsertEventsRequest,
        UpsertEventsResponse,
    },
    store::event::Store,
};

use super::store_error_to_status;

#[derive(Debug)]
pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

#[tonic::async_trait]
impl<StoreType: Store> proto::event_service_server::EventService for Service<StoreType> {
    async fn upsert_events(
        &self,
        request: Request<UpsertEventsRequest>,
    ) -> Result<Response<UpsertEventsResponse>, Status> {
        let events = self
            .store
            .upsert(request.into_inner().events)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(UpsertEventsResponse { events }))
    }

    async fn list_events(
        &self,
        request: Request<ListEventsRequest>,
    ) -> Result<Response<ListEventsResponse>, Status> {
        let events = self
            .store
            .list(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(ListEventsResponse { events }))
    }

    async fn delete_events(
        &self,
        request: Request<proto::DeleteEventsRequest>,
    ) -> Result<Response<DeleteEventsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteEventsResponse {}))
    }
}
