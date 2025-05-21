use crate::{
    api::{
        authorization_state_to_status, common::try_logical_string_query,
        err_missing_claims_context, middleware::authentication::ClaimsContext,
        store_error_to_status, ValidationError,
    },
    proto::{
        self, compound_registration_query, permission_role, registration_query,
        DeleteRegistrationsRequest, DeleteRegistrationsResponse, EventRole, Permission,
        PermissionRole, QueryRegistrationsRequest, QueryRegistrationsResponse, Registration,
        RegistrationQuery, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
    },
    store::{
        permission::Store as PermissionStore,
        registration::{Query, Store},
        CompoundOperator, CompoundQuery,
    },
};
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};

pub struct Service<StoreType: Store, PermissionStoreType: PermissionStore> {
    store: Arc<StoreType>,
    permission_store: Arc<PermissionStoreType>,
}

impl<StoreType: Store, PermissionStoreType: PermissionStore>
    Service<StoreType, PermissionStoreType>
{
    pub fn new(store: Arc<StoreType>, permission_store: Arc<PermissionStoreType>) -> Self {
        Service {
            store,
            permission_store,
        }
    }
}

fn validate_registration(registration: &Registration) -> Result<(), ValidationError> {
    if registration.event_id.is_empty() {
        return Err(ValidationError::new_empty("event_id"));
    }

    Ok(())
}

fn try_parse_registration_query(query: RegistrationQuery) -> Result<Query, ValidationError> {
    match query.query {
        Some(registration_query::Query::EventId(event_id_query)) => Ok(Query::EventId(
            try_logical_string_query(event_id_query)
                .map_err(|e| e.with_context("query.event_id"))?,
        )),

        Some(registration_query::Query::Id(id_query)) => Ok(Query::Id(
            try_logical_string_query(id_query).map_err(|e| e.with_context("query.id"))?,
        )),

        Some(registration_query::Query::Compound(compound_query)) => {
            let operator =
                match compound_registration_query::Operator::try_from(compound_query.operator) {
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
                    try_parse_registration_query(query).map_err(|e: ValidationError| {
                        e.with_context(&format!("query.compound.queries[{}]", i))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Query::Compound(CompoundQuery { operator, queries }))
        }
        None => Err(ValidationError::new_empty("query")),
    }
}

fn upsert_permissions(user_id: &str, registrations: &[Registration]) -> Vec<Permission> {
    registrations
        .iter()
        .flat_map(|registration| {
            vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: registration.event_id.clone(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: registration.event_id.clone(),
                        })),
                    }),
                },
            ]
        })
        .collect::<Vec<_>>()
}

fn query_permissions(user_id: &str, registrations: &[Registration]) -> Vec<Permission> {
    registrations
        .iter()
        .map(|registration| Permission {
            id: "".to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventViewer(EventRole {
                    event_id: registration.event_id.clone(),
                })),
            }),
        })
        .collect::<Vec<_>>()
}

fn delete_permissions(user_id: &str, registration_ids: &[String]) -> Vec<Permission> {
    registration_ids
        .iter()
        .flat_map(|registration_id| {
            vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: registration_id.clone(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: registration_id.clone(),
                        })),
                    }),
                },
            ]
        })
        .collect::<Vec<_>>()
}

#[tonic::async_trait]
impl<StoreType: Store, PermissionStoreType: PermissionStore>
    proto::registration_service_server::RegistrationService
    for Service<StoreType, PermissionStoreType>
{
    async fn upsert_registrations(
        &self,
        request: Request<UpsertRegistrationsRequest>,
    ) -> Result<Response<UpsertRegistrationsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let request_registrations = request.registrations;

        for (idx, registration) in request_registrations.iter().enumerate() {
            validate_registration(registration).map_err(|e| -> Status {
                e.with_context(&format!("registrations[{}]", idx)).into()
            })?;
        }

        // Check if user has EventEditor permission for all registrations
        let required_permissions =
            upsert_permissions(&claims_context.claims.sub, &request_registrations);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        let registrations = self
            .store
            .upsert(request_registrations)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(UpsertRegistrationsResponse { registrations }))
    }

    async fn query_registrations(
        &self,
        request: Request<QueryRegistrationsRequest>,
    ) -> Result<Response<QueryRegistrationsResponse>, Status> {
        let (_, extensions, query_request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let query = query_request
            .query
            .map(|query| try_parse_registration_query(query))
            .transpose()?;

        let registrations = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        // Check permissions for all registrations returned by the query
        let required_permissions = query_permissions(&claims_context.claims.sub, &registrations);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        // Create a set of event IDs that the user doesn't have permission to view
        let hidden_events = failed_permissions
            .into_iter()
            .filter_map(|permission| {
                if let Some(role) = &permission.role {
                    if let Some(permission_role::Role::EventViewer(event_role)) = &role.role {
                        return Some(event_role.event_id.clone());
                    }
                }
                None
            })
            .collect::<HashSet<_>>();

        // Filter registrations to only include those the user has permission to view
        let filtered_registrations = registrations
            .into_iter()
            .filter(|registration| !hidden_events.contains(&registration.event_id))
            .collect::<Vec<_>>();

        Ok(Response::new(QueryRegistrationsResponse {
            registrations: filtered_registrations,
        }))
    }

    async fn delete_registrations(
        &self,
        request: Request<DeleteRegistrationsRequest>,
    ) -> Result<Response<DeleteRegistrationsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let registration_ids = request.ids;

        // Check if user has EventEditor permission for all registrations to be deleted
        let required_permissions =
            delete_permissions(&claims_context.claims.sub, &registration_ids);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        self.store
            .delete(&registration_ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteRegistrationsResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use super::Service;
    use crate::{
        api::middleware::authentication::ClaimsContext,
        authentication::Claims,
        proto::{
            permission_role, registration_service_server::RegistrationService, DeleteRegistrationsRequest,
            DeleteRegistrationsResponse, EventRole, Permission, PermissionRole, QueryRegistrationsRequest, 
            Registration, RegistrationItem, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
        },
        store::{
            permission::MockStore as MockPermissionStore,
            registration::MockStore as MockRegistrationStore,
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
        NotFound,
    }

    #[test_case(UpsertTest::Success; "success")]
    #[test_case(UpsertTest::PermissionDenied; "permission_denied")]
    #[test_case(UpsertTest::NotFound; "not_found")]
    #[tokio::test]
    async fn upsert(test_name: UpsertTest) {
        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Result<UpsertRegistrationsResponse, Status>,
        }

        let registration_id = "registration_id";
        let user_id = "user_id";
        let event_id = "event_id";

        let registration_item = RegistrationItem {
            schema_item_id: "schema_item_id".to_string(),
            value: "value".to_string(),
        };

        let registration = Registration {
            id: "".to_string(),
            event_id: event_id.to_string(),
            items: vec![registration_item],
        };

        let mut returned_registration = registration.clone();
        returned_registration.id = registration_id.to_string();

        let tc = match test_name {
            UpsertTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(UpsertRegistrationsResponse {
                    registrations: vec![returned_registration.clone()],
                }),
            },
            UpsertTest::PermissionDenied => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: event_id.to_string(),
                        })),
                    }),
                }],
                result: Err(Status::permission_denied("")),
            },
            UpsertTest::NotFound => TestCase {
                missing_permissions: vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventEditor(EventRole {
                                event_id: event_id.to_string(),
                            })),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventViewer(EventRole {
                                event_id: event_id.to_string(),
                            })),
                        }),
                    },
                ],
                result: Err(Status::not_found(event_id.to_string())),
            },
        };

        let mut registration_store = MockRegistrationStore::new();
        let mut permission_store = MockPermissionStore::new();

        registration_store
            .expect_upsert()
            .with(eq(vec![registration.clone()]))
            .returning(move |_| {
                let returned_registration = returned_registration.clone();
                Box::pin(async move { Ok(vec![returned_registration]) })
            });

        permission_store
            .expect_permission_check()
            .with(eq(vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: event_id.to_string(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: event_id.to_string(),
                        })),
                    }),
                },
            ]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(registration_store), Arc::new(permission_store));

        let mut request = Request::new(UpsertRegistrationsRequest {
            registrations: vec![registration],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service
            .upsert_registrations(request)
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
        let registration_id = "registration_id";
        let user_id = "user_id";
        let event_id = "event_id";

        let registration_item = RegistrationItem {
            schema_item_id: "schema_item_id".to_string(),
            value: "value".to_string(),
        };

        let registration = Registration {
            id: registration_id.to_string(),
            event_id: event_id.to_string(),
            items: vec![registration_item],
        };

        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Vec<Registration>,
        }

        let tc = match test_name {
            QueryTest::Success => TestCase {
                missing_permissions: vec![],
                result: vec![registration.clone()],
            },
            QueryTest::Filtered => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: event_id.to_string(),
                        })),
                    }),
                }],
                result: vec![],
            },
        };

        let mut permission_store = MockPermissionStore::new();
        let mut registration_store = MockRegistrationStore::new();

        registration_store.expect_query().returning(move |_| {
            let registration = registration.clone();
            Box::pin(async move { Ok(vec![registration]) })
        });

        permission_store
            .expect_permission_check()
            .with(eq(vec![Permission {
                id: "".to_string(),
                user_id: user_id.to_string(),
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::EventViewer(EventRole {
                        event_id: event_id.to_string(),
                    })),
                }),
            }]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(registration_store), Arc::new(permission_store));

        let mut request = Request::new(QueryRegistrationsRequest { query: None });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.query_registrations(request).await.unwrap();

        assert_eq!(response.into_inner().registrations, tc.result);
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
            result: Result<DeleteRegistrationsResponse, Status>,
        }

        let registration_id = "registration_id";
        let user_id = "user_id";
        let event_id = "event_id";

        let tc = match test_name {
            DeleteTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(DeleteRegistrationsResponse::default()),
            },
            DeleteTest::PermissionDenied => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: event_id.to_string(),
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
                            role: Some(permission_role::Role::EventEditor(EventRole {
                                event_id: event_id.to_string(),
                            })),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventViewer(EventRole {
                                event_id: event_id.to_string(),
                            })),
                        }),
                    },
                ],
                result: Err(Status::not_found(event_id.to_string())),
            },
        };

        let mut registration_store = MockRegistrationStore::new();
        let mut permission_store = MockPermissionStore::new();

        registration_store
            .expect_delete()
            .with(eq(vec![event_id.to_string()]))
            .returning(|_| Box::pin(async { Ok(()) }));

        permission_store
            .expect_permission_check()
            .with(eq(vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: event_id.to_string(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: event_id.to_string(),
                        })),
                    }),
                },
            ]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(registration_store), Arc::new(permission_store));

        let mut request = Request::new(DeleteRegistrationsRequest {
            ids: vec![event_id.to_string()],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.delete_registrations(request).await.map(|r| r.into_inner());

        assert_eq!(
            response.map_err(StatusCompare::new),
            tc.result.map_err(StatusCompare::new)
        );
    }
}
