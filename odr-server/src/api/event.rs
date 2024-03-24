use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::store::{
    event::{Query, Store},
    CompoundOperator, CompoundQuery,
};
use common::proto::{
    self, compound_event_query, event_query, DeleteEventsResponse, EventQuery, QueryEventsRequest,
    QueryEventsResponse, UpsertEventsRequest, UpsertEventsResponse,
};

use super::{common::try_logical_string_query, ValidationError};

#[derive(Debug)]
pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

impl TryFrom<EventQuery> for Query {
    type Error = ValidationError;

    fn try_from(query: EventQuery) -> Result<Self, Self::Error> {
        match query.query {
            Some(event_query::Query::Id(query)) => Ok(Query::Id(
                try_logical_string_query(query).map_err(|e| e.with_context("query.id"))?,
            )),

            Some(event_query::Query::OrganizationId(query)) => Ok(Query::Organization(
                try_logical_string_query(query)
                    .map_err(|e| e.with_context("query.organization_id"))?,
            )),

            Some(event_query::Query::Compound(compound_query)) => {
                let operator =
                    match compound_event_query::Operator::try_from(compound_query.operator) {
                        Ok(compound_event_query::Operator::And) => CompoundOperator::And,
                        Ok(compound_event_query::Operator::Or) => CompoundOperator::Or,
                        Err(_) => {
                            return Err(ValidationError::new_invalid_enum(
                                "query.compound.operator",
                            ))
                        }
                    };

                let queries = compound_query
                    .queries
                    .into_iter()
                    .enumerate()
                    .map(|(idx, query)| {
                        query.try_into().map_err(|e: Self::Error| {
                            e.with_context(&format!("query.compound.queries[{}]", idx))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Query::CompoundQuery(CompoundQuery { operator, queries }))
            }
            None => Err(ValidationError::new_empty("query")),
        }
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
            .map_err(|e| -> Status { e.into() })?;
        Ok(Response::new(UpsertEventsResponse { events }))
    }

    async fn query_events(
        &self,
        request: Request<QueryEventsRequest>,
    ) -> Result<Response<QueryEventsResponse>, Status> {
        let query = request.into_inner().query;
        let query = query.map(|query| query.try_into()).transpose()?;

        let events = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { e.into() })?;
        Ok(Response::new(QueryEventsResponse { events }))
    }

    async fn delete_events(
        &self,
        request: Request<proto::DeleteEventsRequest>,
    ) -> Result<Response<DeleteEventsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(DeleteEventsResponse {}))
    }
}
