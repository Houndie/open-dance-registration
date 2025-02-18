use crate::{
    api::{common::try_logical_string_query, store_error_to_status, ValidationError},
    proto::{
        self, compound_event_query, event_query, DeleteEventsResponse, EventQuery,
        QueryEventsRequest, QueryEventsResponse, UpsertEventsRequest, UpsertEventsResponse,
    },
    store::{
        event::{Query, Store},
        CompoundOperator, CompoundQuery,
    },
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn try_parse_event_query(query: EventQuery) -> Result<Query, ValidationError> {
    match query.query {
        Some(event_query::Query::Id(query)) => Ok(Query::Id(
            try_logical_string_query(query).map_err(|e| e.with_context("query.id"))?,
        )),

        Some(event_query::Query::OrganizationId(query)) => Ok(Query::Organization(
            try_logical_string_query(query).map_err(|e| e.with_context("query.organization_id"))?,
        )),

        Some(event_query::Query::Compound(compound_query)) => {
            let operator = match compound_event_query::Operator::try_from(compound_query.operator) {
                Ok(compound_event_query::Operator::And) => CompoundOperator::And,
                Ok(compound_event_query::Operator::Or) => CompoundOperator::Or,
                Err(_) => return Err(ValidationError::new_invalid_enum("query.compound.operator")),
            };

            let queries = compound_query
                .queries
                .into_iter()
                .enumerate()
                .map(|(idx, query)| {
                    try_parse_event_query(query).map_err(|e: ValidationError| {
                        e.with_context(&format!("query.compound.queries[{}]", idx))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Query::CompoundQuery(CompoundQuery { operator, queries }))
        }
        None => Err(ValidationError::new_empty("query")),
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
            .map_err(|e| -> Status { store_error_to_status(e) })?;
        Ok(Response::new(UpsertEventsResponse { events }))
    }

    async fn query_events(
        &self,
        request: Request<QueryEventsRequest>,
    ) -> Result<Response<QueryEventsResponse>, Status> {
        let query = request.into_inner().query;
        let query = query
            .map(|query| try_parse_event_query(query))
            .transpose()?;

        let events = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;
        Ok(Response::new(QueryEventsResponse { events }))
    }

    async fn delete_events(
        &self,
        request: Request<proto::DeleteEventsRequest>,
    ) -> Result<Response<DeleteEventsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteEventsResponse {}))
    }
}
