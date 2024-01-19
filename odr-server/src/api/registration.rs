use std::sync::Arc;

use common::proto::{
    self, DeleteRegistrationsRequest, DeleteRegistrationsResponse, ListRegistrationsRequest,
    ListRegistrationsResponse, QueryRegistrationsRequest, QueryRegistrationsResponse,
    UpsertRegistrationsRequest, UpsertRegistrationsResponse,
};
use tonic::{Request, Response, Status};

use crate::store::registration::Store;

use super::store_error_to_status;

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

#[tonic::async_trait]
impl<StoreType: Store> proto::registration_service_server::RegistrationService
    for Service<StoreType>
{
    async fn upsert_registrations(
        &self,
        request: Request<UpsertRegistrationsRequest>,
    ) -> Result<Response<UpsertRegistrationsResponse>, Status> {
        let registrations = self
            .store
            .upsert(request.into_inner().registrations)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(UpsertRegistrationsResponse { registrations }))
    }

    async fn list_registrations(
        &self,
        request: Request<ListRegistrationsRequest>,
    ) -> Result<Response<ListRegistrationsResponse>, Status> {
        let registrations = self
            .store
            .list(request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(ListRegistrationsResponse { registrations }))
    }

    async fn query_registrations(
        &self,
        request: Request<QueryRegistrationsRequest>,
    ) -> Result<Response<QueryRegistrationsResponse>, Status> {
        let registrations = self
            .store
            .query(request.into_inner().query.unwrap())
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(QueryRegistrationsResponse { registrations }))
    }

    async fn delete_registrations(
        &self,
        request: Request<DeleteRegistrationsRequest>,
    ) -> Result<Response<DeleteRegistrationsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteRegistrationsResponse {}))
    }
}
