use std::sync::Arc;

use common::proto::{
    self, compound_registration_query, registration_query, DeleteRegistrationsRequest,
    DeleteRegistrationsResponse, ListRegistrationsRequest, ListRegistrationsResponse,
    QueryRegistrationsRequest, QueryRegistrationsResponse, Registration, RegistrationItem,
    RegistrationQuery, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
};
use tonic::{Request, Response, Status};

use crate::store::registration::Store;

use super::{store_error_to_status, ValidationError};

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn validate_registration_item(item: &RegistrationItem) -> Result<(), ValidationError> {
    if item.value.is_none() {
        return Err(ValidationError::new_empty("value"));
    };

    Ok(())
}

fn validate_registration(registration: &Registration) -> Result<(), ValidationError> {
    if registration.event_id.is_empty() {
        return Err(ValidationError::new_empty("event_id"));
    }

    for (i, item) in registration.items.iter().enumerate() {
        validate_registration_item(item).map_err(|e| e.with_context(&format!("items[{}]", i)))?;
    }

    Ok(())
}

fn validate_query(query: &RegistrationQuery) -> Result<(), ValidationError> {
    let query = match &query.query {
        Some(query) => query,
        None => return Err(ValidationError::new_empty("query")),
    };

    match query {
        registration_query::Query::EventId(event_id_query) => {
            if event_id_query.operator.is_none() {
                return Err(ValidationError::new_empty("query.event_id.operator"));
            }
        }
        registration_query::Query::Compound(compound_query) => {
            if compound_registration_query::Operator::try_from(compound_query.operator).is_err() {
                return Err(ValidationError::new_invalid_enum("query.compound.operator"));
            }

            let left = match &compound_query.left {
                Some(left) => left,
                None => return Err(ValidationError::new_empty("query.compound.left")),
            };

            validate_query(left).map_err(|e| e.with_context("query.compound.left"))?;

            let right = match &compound_query.right {
                Some(right) => right,
                None => return Err(ValidationError::new_empty("query.compound.right")),
            };

            validate_query(right).map_err(|e| e.with_context("query.compound.right"))?;
        }
    }

    Ok(())
}

#[tonic::async_trait]
impl<StoreType: Store> proto::registration_service_server::RegistrationService
    for Service<StoreType>
{
    async fn upsert_registrations(
        &self,
        request: Request<UpsertRegistrationsRequest>,
    ) -> Result<Response<UpsertRegistrationsResponse>, Status> {
        let request_registrations = request.into_inner().registrations;

        for (idx, registration) in request_registrations.iter().enumerate() {
            validate_registration(registration).map_err(|e| -> Status {
                e.with_context(&format!("registrations[{}]", idx)).into()
            })?;
        }

        let registrations = self
            .store
            .upsert(request_registrations)
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
        let query = request.into_inner().query;

        let query = match query {
            Some(query) => query,
            None => return Err(ValidationError::new_empty("query").into()),
        };

        validate_query(&query).map_err(|e| -> Status { e.with_context("query").into() })?;

        let registrations = self
            .store
            .query(query)
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
