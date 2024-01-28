use std::sync::Arc;

use common::proto::{
    self, compound_registration_query, registration_query, DeleteRegistrationsRequest,
    DeleteRegistrationsResponse, QueryRegistrationsRequest, QueryRegistrationsResponse,
    Registration, RegistrationItem, RegistrationQuery, UpsertRegistrationsRequest,
    UpsertRegistrationsResponse,
};
use tonic::{Request, Response, Status};

use crate::store::{
    registration::{Query, Store},
    CompoundOperator, CompoundQuery,
};

use super::{
    common::{to_logical_string_query, validate_string_query},
    store_error_to_status, ValidationError,
};

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
            validate_string_query(event_id_query).map_err(|e| e.with_context("query.event_id"))?;
        }
        registration_query::Query::Id(query) => {
            validate_string_query(query).map_err(|e| e.with_context("query.id"))?;
        }
        registration_query::Query::Compound(compound_query) => {
            if compound_registration_query::Operator::try_from(compound_query.operator).is_err() {
                return Err(ValidationError::new_invalid_enum("query.compound.operator"));
            }

            for (i, query) in compound_query.queries.iter().enumerate() {
                validate_query(query)
                    .map_err(|e| e.with_context(&format!("query.compound.queries[{}]", i)))?;
            }
        }
    }

    Ok(())
}

impl From<RegistrationQuery> for Query {
    fn from(query: RegistrationQuery) -> Self {
        match query.query.unwrap() {
            registration_query::Query::EventId(event_id_query) => {
                Query::EventId(to_logical_string_query(event_id_query))
            }

            registration_query::Query::Id(id_query) => Query::Id(to_logical_string_query(id_query)),

            registration_query::Query::Compound(compound_query) => {
                let operator =
                    match compound_registration_query::Operator::try_from(compound_query.operator)
                        .unwrap()
                    {
                        compound_registration_query::Operator::And => CompoundOperator::And,
                        compound_registration_query::Operator::Or => CompoundOperator::Or,
                    };

                let queries = compound_query
                    .queries
                    .into_iter()
                    .map(|query| query.into())
                    .collect();

                Query::Compound(CompoundQuery { operator, queries })
            }
        }
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

    async fn query_registrations(
        &self,
        request: Request<QueryRegistrationsRequest>,
    ) -> Result<Response<QueryRegistrationsResponse>, Status> {
        let query = request.into_inner().query;

        let registrations = match query {
            Some(query) => {
                validate_query(&query).map_err(|e| -> Status { e.with_context("query").into() })?;

                let store_query = query.into();

                self.store
                    .query(store_query)
                    .await
                    .map_err(|e| store_error_to_status(e))?
            }
            None => self
                .store
                .list()
                .await
                .map_err(|e| store_error_to_status(e))?,
        };

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
