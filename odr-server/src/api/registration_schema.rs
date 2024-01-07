use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::{
    proto::{
        self, DeleteRegistrationSchemasResponse, ListRegistrationSchemasRequest,
        ListRegistrationSchemasResponse, UpsertRegistrationSchemasRequest,
        UpsertRegistrationSchemasResponse,
    },
    store::registration_schema::Store,
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
impl<StoreType: Store> proto::registration_schema_service_server::RegistrationSchemaService
    for Service<StoreType>
{
    async fn upsert_registration_schemas(
        &self,
        request: Request<UpsertRegistrationSchemasRequest>,
    ) -> Result<Response<UpsertRegistrationSchemasResponse>, Status> {
        let registration_schemas = self
            .store
            .upsert(request.into_inner().registration_schemas)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(UpsertRegistrationSchemasResponse {
            registration_schemas,
        }))
    }

    async fn list_registration_schemas(
        &self,
        request: Request<ListRegistrationSchemasRequest>,
    ) -> Result<Response<ListRegistrationSchemasResponse>, Status> {
        let registration_schemas = self
            .store
            .list(request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(ListRegistrationSchemasResponse {
            registration_schemas,
        }))
    }

    async fn delete_registration_schemas(
        &self,
        request: Request<proto::DeleteRegistrationSchemasRequest>,
    ) -> Result<Response<DeleteRegistrationSchemasResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteRegistrationSchemasResponse {}))
    }
}
