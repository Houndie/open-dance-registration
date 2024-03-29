use std::sync::Arc;

use common::proto::{
    self, compound_organization_query, organization_query, DeleteOrganizationsRequest,
    DeleteOrganizationsResponse, OrganizationQuery, QueryOrganizationsRequest,
    QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
};
use tonic::{Request, Response, Status};

use crate::store::{
    organization::{Query, Store},
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

impl TryFrom<OrganizationQuery> for Query {
    type Error = ValidationError;

    fn try_from(query: OrganizationQuery) -> Result<Self, Self::Error> {
        match query.query {
            Some(organization_query::Query::Id(query)) => Ok(Query::Id(
                try_logical_string_query(query).map_err(|e| e.with_context("query.id"))?,
            )),

            Some(organization_query::Query::Compound(compound_query)) => {
                let operator = match compound_organization_query::Operator::try_from(
                    compound_query.operator,
                ) {
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
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(UpsertOrganizationsResponse { organizations }))
    }

    async fn query_organizations(
        &self,
        request: Request<QueryOrganizationsRequest>,
    ) -> Result<Response<QueryOrganizationsResponse>, Status> {
        let query = request.into_inner().query;
        let query = query.map(|query| query.try_into()).transpose()?;

        let organizations = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(QueryOrganizationsResponse { organizations }))
    }

    async fn delete_organizations(
        &self,
        request: Request<DeleteOrganizationsRequest>,
    ) -> Result<Response<DeleteOrganizationsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(DeleteOrganizationsResponse {}))
    }
}
