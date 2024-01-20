use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::store::event::Store;
use common::proto::{
    self, DeleteEventsResponse, ListEventsRequest, ListEventsResponse, UpsertEventsRequest,
    UpsertEventsResponse,
};

use super::{store_error_to_status, ValidationError};

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
        let events = request.into_inner().events;
        for (idx, event) in events.iter().enumerate() {
            if event.organization_id == "" {
                return Err(ValidationError::new_empty(&format!(
                    "events[{}].organization_id",
                    idx
                ))
                .into());
            }
        }

        let events = self
            .store
            .upsert(events)
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
