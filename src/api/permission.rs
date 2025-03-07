use crate::{
    api::{
        authorization_state_to_status, common::try_logical_string_query,
        err_missing_claims_context, middleware::authentication::ClaimsContext,
        store_error_to_status, ValidationError,
    },
    proto::{
        compound_permission_query, permission_query, permission_role, permission_role::Role,
        permission_role_query, permission_service_server::PermissionService,
        DeletePermissionsRequest, DeletePermissionsResponse, Permission, PermissionQuery,
        PermissionRole, QueryPermissionsRequest, QueryPermissionsResponse,
        UpsertPermissionsRequest, UpsertPermissionsResponse,
    },
    store::{
        permission::{IdQuery, PermissionRoleQuery, Query, Store},
        CompoundOperator, CompoundQuery,
    },
};
use std::{collections::HashSet, sync::Arc};
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

    let role = match &role.role {
        Some(role) => role,
        None => return Err(ValidationError::new_empty("role.role")),
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
        Some(permission_query::Query::Role(role_query)) => {
            Ok(Query::Role(match role_query.operator {
                Some(permission_role_query::Operator::Is(is)) => {
                    if is.role.is_none() {
                        return Err(ValidationError::new_empty("query.role.operator.role"));
                    }

                    PermissionRoleQuery::Is(is)
                }
                Some(permission_role_query::Operator::IsNot(is_not)) => {
                    if is_not.role.is_none() {
                        return Err(ValidationError::new_empty("query.role.operator.role"));
                    }

                    PermissionRoleQuery::IsNot(is_not)
                }
                None => return Err(ValidationError::new_empty("query.role.operator")),
            }))
        }
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

fn required_permissions(user_id: &str, permissions: &[Permission]) -> Vec<Permission> {
    permissions
        .iter()
        .map(|p| match p.role.as_ref().unwrap().role.as_ref().unwrap() {
            permission_role::Role::ServerAdmin(_) => vec![Permission {
                id: "".to_string(),
                user_id: user_id.to_string(),
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::ServerAdmin(())),
                }),
            }],
            permission_role::Role::OrganizationAdmin(r)
            | permission_role::Role::OrganizationViewer(r) => {
                vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationAdmin(r.clone())),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationViewer(r.clone())),
                        }),
                    },
                ]
            }
            permission_role::Role::EventAdmin(r)
            | permission_role::Role::EventEditor(r)
            | permission_role::Role::EventViewer(r) => {
                vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventAdmin(r.clone())),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventViewer(r.clone())),
                        }),
                    },
                ]
            }
        })
        .flatten()
        .collect::<Vec<_>>()
}

#[tonic::async_trait]
impl<StoreType: Store> PermissionService for Service<StoreType> {
    async fn upsert_permissions(
        &self,
        request: Request<UpsertPermissionsRequest>,
    ) -> Result<Response<UpsertPermissionsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let request_permissions = request.permissions;

        for (i, permission) in request_permissions.iter().enumerate() {
            validate_permission(permission)
                .map_err(|e| -> Status { e.with_context(&format!("permissions[{}]", i)).into() })?
        }

        let required_permissions =
            required_permissions(&claims_context.claims.sub, &request_permissions);

        let failed_permissions = self
            .store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

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
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(|| Status::unauthenticated("Missing claims context"))?;

        let query = request.query.map(try_parse_query).transpose()?;

        let permissions = self
            .store
            .query(query.as_ref())
            .await
            .map_err(store_error_to_status)?;

        let required_permissions = required_permissions(&claims_context.claims.sub, &permissions);

        let failed_permissions = self
            .store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        let mut hidden_organizations = HashSet::new();
        let mut hidden_events = HashSet::new();
        let mut can_see_server_admin = true;

        for permission in failed_permissions.into_iter() {
            match permission.role.unwrap().role.unwrap() {
                permission_role::Role::ServerAdmin(_) => {
                    can_see_server_admin = false;
                }
                permission_role::Role::OrganizationAdmin(o)
                | permission_role::Role::OrganizationViewer(o) => {
                    hidden_organizations.insert(o.organization_id);
                }
                permission_role::Role::EventAdmin(e)
                | permission_role::Role::EventEditor(e)
                | permission_role::Role::EventViewer(e) => {
                    hidden_events.insert(e.event_id);
                }
            }
        }

        let permissions = permissions
            .into_iter()
            .filter(
                |permission| match permission.role.as_ref().unwrap().role.as_ref().unwrap() {
                    permission_role::Role::ServerAdmin(_) => can_see_server_admin,
                    permission_role::Role::OrganizationAdmin(o)
                    | permission_role::Role::OrganizationViewer(o) => {
                        !hidden_organizations.contains(&o.organization_id)
                    }
                    permission_role::Role::EventAdmin(e)
                    | permission_role::Role::EventEditor(e)
                    | permission_role::Role::EventViewer(e) => !hidden_events.contains(&e.event_id),
                },
            )
            .collect::<Vec<_>>();

        return Ok(Response::new(QueryPermissionsResponse { permissions }));
    }

    async fn delete_permissions(
        &self,
        request: Request<DeletePermissionsRequest>,
    ) -> Result<Response<DeletePermissionsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(|| Status::unauthenticated("Missing claims context"))?;

        let ids = request.ids;

        let to_be_deleted = self
            .store
            .query(Some(&Query::CompoundQuery(CompoundQuery {
                operator: CompoundOperator::Or,
                queries: ids
                    .iter()
                    .map(|id| Query::Id(IdQuery::Equals(id.clone())))
                    .collect(),
            })))
            .await
            .map_err(store_error_to_status)?;

        let required_permissions = required_permissions(&claims_context.claims.sub, &to_be_deleted);

        let failed_permissions = self
            .store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        self.store
            .delete(&ids)
            .await
            .map_err(store_error_to_status)?;

        return Ok(Response::new(DeletePermissionsResponse {}));
    }
}
