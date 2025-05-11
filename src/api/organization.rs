use crate::{
    api::{
        authorization_state_to_status, common::try_logical_string_query,
        err_missing_claims_context, middleware::authentication::ClaimsContext,
        store_error_to_status, ValidationError,
    },
    proto::{
        self, compound_organization_query, organization_query, permission_role,
        DeleteOrganizationsRequest, DeleteOrganizationsResponse, OrganizationQuery,
        OrganizationRole, Permission, PermissionRole, QueryOrganizationsRequest,
        QueryOrganizationsResponse, UpsertOrganizationsRequest, UpsertOrganizationsResponse,
    },
    store::{
        organization::{Query, Store as OrganizationStore},
        permission::Store as PermissionStore,
        CompoundOperator, CompoundQuery,
    },
};
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};

pub struct Service<OrganizationStoreType: OrganizationStore, PermissionStoreType: PermissionStore> {
    organization_store: Arc<OrganizationStoreType>,
    permission_store: Arc<PermissionStoreType>,
}

impl<OrganizationStoreType: OrganizationStore, PermissionStoreType: PermissionStore>
    Service<OrganizationStoreType, PermissionStoreType>
{
    pub fn new(
        organization_store: Arc<OrganizationStoreType>,
        permission_store: Arc<PermissionStoreType>,
    ) -> Self {
        Service {
            organization_store,
            permission_store,
        }
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

fn admin_or_viewer_permissions<OrgIter: IntoIterator<Item = String>>(
    user_id: &str,
    org_ids: OrgIter,
) -> Vec<Permission> {
    org_ids
        .into_iter()
        .map(|org_id| {
            vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: org_id.clone(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationViewer(
                            OrganizationRole {
                                organization_id: org_id,
                            },
                        )),
                    }),
                },
            ]
        })
        .flatten()
        .collect()
}

#[tonic::async_trait]
impl<OrganizationStoreType: OrganizationStore, PermissionStoreType: PermissionStore>
    proto::organization_service_server::OrganizationService
    for Service<OrganizationStoreType, PermissionStoreType>
{
    async fn upsert_organizations(
        &self,
        request: Request<UpsertOrganizationsRequest>,
    ) -> Result<Response<UpsertOrganizationsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let request_organizations = request.organizations;

        let required_permissions = vec![Permission {
            id: "".to_string(),
            user_id: claims_context.claims.sub.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::ServerAdmin(())),
            }),
        }];

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        let organizations = self
            .organization_store
            .upsert(request_organizations)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(UpsertOrganizationsResponse { organizations }))
    }

    async fn query_organizations(
        &self,
        request: Request<QueryOrganizationsRequest>,
    ) -> Result<Response<QueryOrganizationsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let query = request.query;

        let query = query
            .map(|query| try_parse_organization_query(query))
            .transpose()?;

        let organizations = self
            .organization_store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        let required_permissions = admin_or_viewer_permissions(
            &claims_context.claims.sub,
            organizations.iter().map(|org| org.id.clone()),
        );

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        let mut failed_organizations = HashSet::new();
        for permission in failed_permissions {
            if let permission_role::Role::OrganizationAdmin(organization_role) =
                permission.role.unwrap().role.unwrap()
            {
                failed_organizations.insert(organization_role.organization_id);
            }
        }

        let organizations = organizations
            .into_iter()
            .filter(|org| !failed_organizations.contains(&org.id))
            .collect::<Vec<_>>();

        Ok(Response::new(QueryOrganizationsResponse { organizations }))
    }

    async fn delete_organizations(
        &self,
        request: Request<DeleteOrganizationsRequest>,
    ) -> Result<Response<DeleteOrganizationsResponse>, Status> {
        self.organization_store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteOrganizationsResponse {}))
    }
}
