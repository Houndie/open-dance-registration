use crate::{
    api::{common::try_logical_string_query, store_error_to_status, ValidationError},
    proto::{
        compound_permission_query, permission::Role, permission_query,
        permission_service_server::PermissionService, DeletePermissionsRequest,
        DeletePermissionsResponse, Permission, PermissionQuery, QueryPermissionsRequest,
        QueryPermissionsResponse, UpsertPermissionsRequest, UpsertPermissionsResponse,
    },
    store::{
        permission::{Query, Store},
        CompoundOperator, CompoundQuery,
    },
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn validate_permission(permission: &Permission) -> Result<(), ValidationError> {
    if permission.user_id == "" {
        return Err(ValidationError::new_empty("user_id"));
    }

    let role = match &permission.role {
        Some(role) => role,
        None => return Err(ValidationError::new_empty("role")),
    };

    match role {
        Role::ServerAdmin(_) => (),
        Role::OrganizationAdmin(r) => {
            if r.organization_id == "" {
                return Err(ValidationError::new_empty("organization_id"));
            }
        }
        Role::OrganizationViewer(r) => {
            if r.organization_id == "" {
                return Err(ValidationError::new_empty("organization_id"));
            }
        }
        Role::EventAdmin(r) => {
            if r.event_id == "" {
                return Err(ValidationError::new_empty("event_id"));
            }
        }
        Role::EventEditor(r) => {
            if r.event_id == "" {
                return Err(ValidationError::new_empty("event_id"));
            }
        }
        Role::EventViewer(r) => {
            if r.event_id == "" {
                return Err(ValidationError::new_empty("event_id"));
            }
        }
    }

    Ok(())
}

fn try_parse_query(query: PermissionQuery) -> Result<Query, ValidationError> {
    match query.query {
        Some(permission_query::Query::Id(id_query)) => Ok(Query::Id(
            try_logical_string_query(id_query).map_err(|e| e.with_context("query.id"))?,
        )),
        Some(permission_query::Query::UserId(user_id_query)) => Ok(Query::UserId(
            try_logical_string_query(user_id_query).map_err(|e| e.with_context("query.user_id"))?,
        )),
        Some(permission_query::Query::Compound(compound_query)) => {
            let operator =
                match compound_permission_query::Operator::try_from(compound_query.operator) {
                    Ok(compound_permission_query::Operator::And) => CompoundOperator::And,
                    Ok(compound_permission_query::Operator::Or) => CompoundOperator::Or,
                    Err(_) => {
                        return Err(ValidationError::new_invalid_enum("query.compound.operator"))
                    }
                };

            let queries = compound_query
                .queries
                .into_iter()
                .enumerate()
                .map(|(idx, query)| {
                    try_parse_query(query).map_err(|e: ValidationError| {
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
impl<StoreType: Store> PermissionService for Service<StoreType> {
    async fn upsert_permissions(
        &self,
        request: Request<UpsertPermissionsRequest>,
    ) -> Result<Response<UpsertPermissionsResponse>, Status> {
        let request_permissions = request.into_inner().permissions;

        for (i, permission) in request_permissions.iter().enumerate() {
            validate_permission(permission)
                .map_err(|e| -> Status { e.with_context(&format!("permissions[{}]", i)).into() })?
        }

        let permissions = self
            .store
            .upsert(request_permissions)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        return Ok(Response::new(UpsertPermissionsResponse { permissions }));
    }

    async fn query_permissions(
        &self,
        request: Request<QueryPermissionsRequest>,
    ) -> Result<Response<QueryPermissionsResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .map(try_parse_query)
            .transpose()?;

        let permissions = self
            .store
            .query(query.as_ref())
            .await
            .map_err(store_error_to_status)?;

        return Ok(Response::new(QueryPermissionsResponse { permissions }));
    }

    async fn delete_permissions(
        &self,
        request: Request<DeletePermissionsRequest>,
    ) -> Result<Response<DeletePermissionsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(store_error_to_status)?;

        return Ok(Response::new(DeletePermissionsResponse {}));
    }
}
