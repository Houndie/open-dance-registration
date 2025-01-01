use std::sync::Arc;

use common::proto::{
    self, compound_organization_query, organization_query, DeleteOrganizationsRequest,
    DeleteOrganizationsResponse, OrganizationQuery, QueryOrganizationsRequest,
    QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
};
use tonic::{Request, Response, Status};

use odr_core::store::{
    organization::{Query, Store},
    CompoundOperator, CompoundQuery,
};

use super::{common::try_logical_string_query, store_error_to_status, ValidationError};

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn try_parse_organization_query(query: OrganizationQuery) -> Result<Query, ValidationError> {
    match query.query {
        Some(organization_query::Query::Id(query)) => Ok(Query::Id(
            try_logical_string_query(query).map_err(|e| e.with_context("query.id"))?,
        )),

        Some(organization_query::Query::Compound(compound_query)) => {
            let operator =
                match compound_organization_query::Operator::try_from(compound_query.operator) {
                    Ok(compound_organization_query::Operator::And) => CompoundOperator::And,
                    Ok(compound_organization_query::Operator::Or) => CompoundOperator::Or,
                    Err(_) => {
                        return Err(ValidationError::new_invalid_enum("query.compound.operator"))
                    }
                };

            let queries = compound_query
                .queries
                .into_iter()
                .enumerate()
                .map(|(idx, query)| {
                    try_parse_organization_query(query).map_err(|e: ValidationError| {
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
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(UpsertOrganizationsResponse { organizations }))
    }

    async fn query_organizations(
        &self,
        request: Request<QueryOrganizationsRequest>,
    ) -> Result<Response<QueryOrganizationsResponse>, Status> {
        let query = request.into_inner().query;
        let query = query
            .map(|query| try_parse_organization_query(query))
            .transpose()?;

        let organizations = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(QueryOrganizationsResponse { organizations }))
    }

    async fn delete_organizations(
        &self,
        request: Request<DeleteOrganizationsRequest>,
    ) -> Result<Response<DeleteOrganizationsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteOrganizationsResponse {}))
    }
}
