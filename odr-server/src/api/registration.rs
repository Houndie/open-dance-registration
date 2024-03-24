use std::sync::Arc;

use common::proto::{
    self, compound_registration_query, registration_query, DeleteRegistrationsRequest,
    DeleteRegistrationsResponse, QueryRegistrationsRequest, QueryRegistrationsResponse,
    Registration, RegistrationQuery, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
};
use tonic::{Request, Response, Status};

use crate::store::{
    registration::{Query, Store},
    CompoundOperator, CompoundQuery,
};

use super::{common::try_logical_string_query, ValidationError};

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn validate_registration(registration: &Registration) -> Result<(), ValidationError> {
    if registration.event_id.is_empty() {
        return Err(ValidationError::new_empty("event_id"));
    }

    Ok(())
}

impl TryFrom<RegistrationQuery> for Query {
    type Error = ValidationError;

    fn try_from(query: RegistrationQuery) -> Result<Self, Self::Error> {
        match query.query {
            Some(registration_query::Query::EventId(event_id_query)) => Ok(Query::EventId(
                try_logical_string_query(event_id_query)
                    .map_err(|e| e.with_context("query.event_id"))?,
            )),

            Some(registration_query::Query::Id(id_query)) => Ok(Query::Id(
                try_logical_string_query(id_query).map_err(|e| e.with_context("query.id"))?,
            )),

            Some(registration_query::Query::Compound(compound_query)) => {
                let operator = match compound_registration_query::Operator::try_from(
                    compound_query.operator,
                ) {
                    Ok(compound_registration_query::Operator::And) => CompoundOperator::And,
                    Ok(compound_registration_query::Operator::Or) => CompoundOperator::Or,
                    Err(_) => {
                        return Err(ValidationError::new_invalid_enum("query.compound.operator"))
                    }
                };

                let queries = compound_query
                    .queries
                    .into_iter()
                    .enumerate()
                    .map(|(i, query)| {
                        query.try_into().map_err(|e: Self::Error| {
                            e.with_context(&format!("query.compound.queries[{}]", i))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Query::Compound(CompoundQuery { operator, queries }))
            }
            None => Err(ValidationError::new_empty("query")),
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
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(UpsertRegistrationsResponse { registrations }))
    }

    async fn query_registrations(
        &self,
        request: Request<QueryRegistrationsRequest>,
    ) -> Result<Response<QueryRegistrationsResponse>, Status> {
        let query = request.into_inner().query;

        let query = query.map(|query| query.try_into()).transpose()?;

        let registrations = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(QueryRegistrationsResponse { registrations }))
    }

    async fn delete_registrations(
        &self,
        request: Request<DeleteRegistrationsRequest>,
    ) -> Result<Response<DeleteRegistrationsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(DeleteRegistrationsResponse {}))
    }
}
