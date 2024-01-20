use std::sync::Arc;

use common::proto::{
    self, DeleteOrganizationsRequest, DeleteOrganizationsResponse, ListOrganizationsRequest,
    ListOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
};
use tonic::{Request, Response, Status};

use crate::store::organization::Store;

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
impl<StoreType: Store> proto::organization_service_server::OrganizationService
    for Service<StoreType>
{
    async fn upsert_organizations(
        &self,
        request: Request<UpsertOrganizationsRequest>,
    ) -> Result<Response<UpsertOrganizationsResponse>, Status> {
        let request_organizations = request.into_inner().organizations;

        let organizations = self
            .store
            .upsert(request_organizations)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(UpsertOrganizationsResponse { organizations }))
    }

    async fn list_organizations(
        &self,
        request: Request<ListOrganizationsRequest>,
    ) -> Result<Response<ListOrganizationsResponse>, Status> {
        let organizations = self
            .store
            .list(request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(ListOrganizationsResponse { organizations }))
    }

    async fn delete_organizations(
        &self,
        request: Request<DeleteOrganizationsRequest>,
    ) -> Result<Response<DeleteOrganizationsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteOrganizationsResponse {}))
    }
}
