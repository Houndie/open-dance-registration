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

#[cfg(test)]
mod tests {
    use super::Service;
    use crate::{
        api::middleware::authentication::ClaimsContext,
        authentication::Claims,
        proto::{
            permission_role, permission_service_server::PermissionService as _,
            DeletePermissionsRequest, DeletePermissionsResponse, OrganizationRole, Permission,
            PermissionRole, QueryPermissionsRequest, UpsertPermissionsRequest,
            UpsertPermissionsResponse,
        },
        store::permission::MockStore,
    };
    use mockall::predicate::eq;
    use std::sync::Arc;
    use test_case::test_case;
    use tonic::{Request, Status};

    enum InsertTest {
        Success,
        PermissionDenied,
        NotFound,
    }

    #[test_case(InsertTest::Success; "success")]
    #[test_case(InsertTest::PermissionDenied; "permission_denied")]
    #[test_case(InsertTest::NotFound; "not_found")]
    #[tokio::test]
    async fn insert(test_name: InsertTest) {
        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Result<UpsertPermissionsResponse, Status>,
        }

        let new_id = "id";
        let user_id = "user_id";
        let org_id = "org_id";

        let permission = Permission {
            id: "".to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::OrganizationViewer(
                    OrganizationRole {
                        organization_id: org_id.to_string(),
                    },
                )),
            }),
        };

        let mut returned_permission = permission.clone();
        returned_permission.id = new_id.to_string();

        let tc = match test_name {
            InsertTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(UpsertPermissionsResponse {
                    permissions: vec![returned_permission.clone()],
                }),
            },
            InsertTest::PermissionDenied => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: org_id.to_string(),
                        })),
                    }),
                }],
                result: Err(Status::permission_denied("")),
            },
            InsertTest::NotFound => TestCase {
                missing_permissions: vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationAdmin(
                                OrganizationRole {
                                    organization_id: org_id.to_string(),
                                },
                            )),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationViewer(
                                OrganizationRole {
                                    organization_id: org_id.to_string(),
                                },
                            )),
                        }),
                    },
                ],
                result: Err(Status::not_found(org_id.to_string())),
            },
        };

        let mut store = MockStore::new();

        store
            .expect_upsert()
            .with(eq(vec![permission.clone()]))
            .returning(move |_| {
                let returned_permission = returned_permission.clone();
                Box::pin(async move { Ok(vec![returned_permission]) })
            });

        store
            .expect_permission_check()
            .with(eq(vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: org_id.to_string(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationViewer(
                            OrganizationRole {
                                organization_id: org_id.to_string(),
                            },
                        )),
                    }),
                },
            ]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(store));

        let mut request = Request::new(UpsertPermissionsRequest {
            permissions: vec![permission],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.upsert_permissions(request).await;
        let response = response.map(|r| r.into_inner()).map_err(|e| e.to_string());

        assert_eq!(response, tc.result.map_err(|e| e.to_string()));
    }

    enum QueryTest {
        Success,
        Filtered,
    }
    #[test_case(QueryTest::Success; "success")]
    #[test_case(QueryTest::Filtered; "filtered")]
    #[tokio::test]
    async fn query(test_name: QueryTest) {
        let id = "id";
        let user_id = "user_id";
        let org_id = "org_id";

        let permission = Permission {
            id: id.to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                    organization_id: org_id.to_string(),
                })),
            }),
        };

        let missing_permission = Permission {
            id: "".to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                    organization_id: org_id.to_string(),
                })),
            }),
        };

        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Vec<Permission>,
        }
        let tc = match test_name {
            QueryTest::Success => TestCase {
                missing_permissions: vec![],
                result: vec![permission.clone()],
            },
            QueryTest::Filtered => TestCase {
                missing_permissions: vec![missing_permission],
                result: vec![],
            },
        };

        let mut store = MockStore::new();

        store.expect_query().returning(move |_| {
            let permission = permission.clone();
            Box::pin(async move { Ok(vec![permission]) })
        });

        store
            .expect_permission_check()
            .with(eq(vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: org_id.to_string(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationViewer(
                            OrganizationRole {
                                organization_id: org_id.to_string(),
                            },
                        )),
                    }),
                },
            ]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(store));

        let mut request = Request::new(QueryPermissionsRequest { query: None });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.query_permissions(request).await.unwrap();

        assert_eq!(response.into_inner().permissions, tc.result);
    }

    enum DeleteTest {
        Success,
        PermissionDenied,
        NotFound,
    }

    #[test_case(DeleteTest::Success; "success")]
    #[test_case(DeleteTest::PermissionDenied; "permission_denied")]
    #[test_case(DeleteTest::NotFound; "not_found")]
    #[tokio::test]
    async fn delete(test_name: DeleteTest) {
        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Result<DeletePermissionsResponse, Status>,
        }

        let id = "id";
        let user_id = "user_id";
        let org_id = "org_id";

        let permission = Permission {
            id: id.to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::OrganizationViewer(
                    OrganizationRole {
                        organization_id: org_id.to_string(),
                    },
                )),
            }),
        };

        let tc = match test_name {
            DeleteTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(DeletePermissionsResponse::default()),
            },
            DeleteTest::PermissionDenied => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: org_id.to_string(),
                        })),
                    }),
                }],
                result: Err(Status::permission_denied("")),
            },
            DeleteTest::NotFound => TestCase {
                missing_permissions: vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationAdmin(
                                OrganizationRole {
                                    organization_id: org_id.to_string(),
                                },
                            )),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationViewer(
                                OrganizationRole {
                                    organization_id: org_id.to_string(),
                                },
                            )),
                        }),
                    },
                ],
                result: Err(Status::not_found(org_id.to_string())),
            },
        };

        let mut store = MockStore::new();

        store
            .expect_delete()
            .with(eq(vec![id.to_string()]))
            .returning(|_| Box::pin(async { Ok(()) }));

        store
            .expect_permission_check()
            .with(eq(vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: org_id.to_string(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationViewer(
                            OrganizationRole {
                                organization_id: org_id.to_string(),
                            },
                        )),
                    }),
                },
            ]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        store.expect_query().returning(move |_| {
            let permission = permission.clone();
            Box::pin(async move { Ok(vec![permission]) })
        });

        let service = Service::new(Arc::new(store));

        let mut request = Request::new(DeletePermissionsRequest {
            ids: vec![id.to_string()],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.delete_permissions(request).await;
        let response = response.map(|r| r.into_inner()).map_err(|e| e.to_string());

        assert_eq!(response, tc.result.map_err(|e| e.to_string()));
    }
}
