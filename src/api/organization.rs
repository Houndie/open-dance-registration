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

fn delete_permissions(user_id: &str, org_ids: &[String]) -> Vec<Permission> {
    org_ids
        .iter()
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
                        role: Some(permission_role::Role::OrganizationViewer(OrganizationRole {
                            organization_id: org_id.clone(),
                        })),
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
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let org_ids = request.ids;

        // Check permissions
        let required_permissions = delete_permissions(&claims_context.claims.sub, &org_ids);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        self.organization_store
            .delete(&org_ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteOrganizationsResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use super::Service;
    use crate::{
        api::middleware::authentication::ClaimsContext,
        authentication::Claims,
        proto::{
            organization_service_server::OrganizationService as _, permission_role,
            DeleteOrganizationsRequest, DeleteOrganizationsResponse, Organization,
            OrganizationRole, Permission, PermissionRole, QueryOrganizationsRequest,
            UpsertOrganizationsRequest, UpsertOrganizationsResponse,
        },
        store::{
            organization::MockStore as MockOrganizationStore,
            permission::MockStore as MockPermissionStore,
        },
        test_helpers::StatusCompare,
    };
    use mockall::predicate::eq;
    use std::sync::Arc;
    use test_case::test_case;
    use tonic::{Request, Status};

    enum UpsertTest {
        Success,
        PermissionDenied,
    }

    #[test_case(UpsertTest::Success; "success")]
    #[test_case(UpsertTest::PermissionDenied; "permission_denied")]
    #[tokio::test]
    async fn upsert(test_name: UpsertTest) {
        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Result<UpsertOrganizationsResponse, Status>,
        }

        let new_id = "id";
        let user_id = "user_id";

        let organization = Organization {
            id: "".to_string(),
            name: "Test Organization".to_string(),
        };

        let mut returned_organization = organization.clone();
        returned_organization.id = new_id.to_string();

        let tc = match test_name {
            UpsertTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(UpsertOrganizationsResponse {
                    organizations: vec![returned_organization.clone()],
                }),
            },
            UpsertTest::PermissionDenied => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::ServerAdmin(())),
                    }),
                }],
                result: Err(Status::permission_denied("")),
            },
        };

        let mut permission_store = MockPermissionStore::new();
        let mut organization_store = MockOrganizationStore::new();

        organization_store
            .expect_upsert()
            .with(eq(vec![organization.clone()]))
            .returning(move |_| {
                let returned_organization = returned_organization.clone();
                Box::pin(async move { Ok(vec![returned_organization]) })
            });

        permission_store
            .expect_permission_check()
            .with(eq(vec![Permission {
                id: "".to_string(),
                user_id: user_id.to_string(),
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::ServerAdmin(())),
                }),
            }]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(organization_store), Arc::new(permission_store));

        let mut request = Request::new(UpsertOrganizationsRequest {
            organizations: vec![organization],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service
            .upsert_organizations(request)
            .await
            .map(|r| r.into_inner());

        assert_eq!(
            response.map_err(StatusCompare::new),
            tc.result.map_err(StatusCompare::new)
        );
    }

    enum QueryTest {
        Success,
        Filtered,
    }

    #[test_case(QueryTest::Success; "success")]
    #[test_case(QueryTest::Filtered; "filtered")]
    #[tokio::test]
    async fn query(test_name: QueryTest) {
        let id = "org_id";
        let user_id = "user_id";

        let organization = Organization {
            id: id.to_string(),
            name: "Test Organization".to_string(),
        };

        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Vec<Organization>,
        }

        let tc = match test_name {
            QueryTest::Success => TestCase {
                missing_permissions: vec![],
                result: vec![organization.clone()],
            },
            QueryTest::Filtered => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: id.to_string(),
                        })),
                    }),
                }],
                result: vec![],
            },
        };

        let mut permission_store = MockPermissionStore::new();
        let mut organization_store = MockOrganizationStore::new();

        organization_store.expect_query().returning(move |_| {
            let organization = organization.clone();
            Box::pin(async move { Ok(vec![organization]) })
        });

        permission_store
            .expect_permission_check()
            .with(eq(vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id: id.to_string(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::OrganizationViewer(
                            OrganizationRole {
                                organization_id: id.to_string(),
                            },
                        )),
                    }),
                },
            ]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(organization_store), Arc::new(permission_store));

        let mut request = Request::new(QueryOrganizationsRequest { query: None });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.query_organizations(request).await.unwrap();

        assert_eq!(response.into_inner().organizations, tc.result);
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
            result: Result<DeleteOrganizationsResponse, Status>,
        }

        let org_id = "org_id";
        let user_id = "user_id";

        let tc = match test_name {
            DeleteTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(DeleteOrganizationsResponse::default()),
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

        let mut organization_store = MockOrganizationStore::new();
        let mut permission_store = MockPermissionStore::new();

        organization_store
            .expect_delete()
            .with(eq(vec![org_id.to_string()]))
            .returning(|_| Box::pin(async { Ok(()) }));

        permission_store
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

        let service = Service::new(Arc::new(organization_store), Arc::new(permission_store));

        let mut request = Request::new(DeleteOrganizationsRequest {
            ids: vec![org_id.to_string()],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service
            .delete_organizations(request)
            .await
            .map(|r| r.into_inner());

        assert_eq!(
            response.map_err(StatusCompare::new),
            tc.result.map_err(StatusCompare::new)
        );
    }
}
