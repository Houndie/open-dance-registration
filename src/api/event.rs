use crate::{
    api::{
        authorization_state_to_status, common::try_logical_string_query,
        err_missing_claims_context, middleware::authentication::ClaimsContext,
        store_error_to_status, ValidationError,
    },
    proto::{
        self, compound_event_query, event_query, permission_role, DeleteEventsResponse, Event,
        EventQuery, EventRole, OrganizationRole, Permission, PermissionRole, QueryEventsRequest,
        QueryEventsResponse, UpsertEventsRequest, UpsertEventsResponse,
    },
    store::{
        event::{Query, Store as EventStore},
        permission::Store as PermissionStore,
        CompoundOperator, CompoundQuery,
    },
};
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct Service<EventStoreType: EventStore, PermissionStoreType: PermissionStore> {
    event_store: Arc<EventStoreType>,
    permission_store: Arc<PermissionStoreType>,
}

impl<EventStoreType: EventStore, PermissionStoreType: PermissionStore>
    Service<EventStoreType, PermissionStoreType>
{
    pub fn new(
        event_store: Arc<EventStoreType>,
        permission_store: Arc<PermissionStoreType>,
    ) -> Self {
        Service {
            event_store,
            permission_store,
        }
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

fn upsert_permissions(user_id: &str, events: &[Event]) -> Vec<Permission> {
    events
        .iter()
        .map(|event| {
            if event.id.is_empty() {
                // New event - needs organization-level permissions
                vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::OrganizationAdmin(
                                OrganizationRole {
                                    organization_id: event.organization_id.clone(),
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
                                    organization_id: event.organization_id.clone(),
                                },
                            )),
                        }),
                    },
                ]
            } else {
                // Existing event - event-level permissions are sufficient
                vec![
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventEditor(EventRole {
                                event_id: event.id.clone(),
                            })),
                        }),
                    },
                    Permission {
                        id: "".to_string(),
                        user_id: user_id.to_string(),
                        role: Some(PermissionRole {
                            role: Some(permission_role::Role::EventViewer(EventRole {
                                event_id: event.id.clone(),
                            })),
                        }),
                    },
                ]
            }
        })
        .flatten()
        .collect::<Vec<_>>()
}

fn query_permissions(user_id: &str, events: &[Event]) -> Vec<Permission> {
    events
        .iter()
        .map(|event| Permission {
            id: "".to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventViewer(EventRole {
                    event_id: event.id.clone(),
                })),
            }),
        })
        .collect::<Vec<_>>()
}

fn delete_permissions(user_id: &str, event_ids: &[String]) -> Vec<Permission> {
    event_ids
        .iter()
        .map(|event_id| Permission {
            id: "".to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventAdmin(EventRole {
                    event_id: event_id.clone(),
                })),
            }),
        })
        .collect::<Vec<_>>()
}

#[tonic::async_trait]
impl<EventStoreType: EventStore, PermissionStoreType: PermissionStore>
    proto::event_service_server::EventService for Service<EventStoreType, PermissionStoreType>
{
    async fn upsert_events(
        &self,
        request: Request<UpsertEventsRequest>,
    ) -> Result<Response<UpsertEventsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let events = request.events;
        for (idx, event) in events.iter().enumerate() {
            if event.organization_id == "" {
                return Err(ValidationError::new_empty(&format!(
                    "events[{}].organization_id",
                    idx
                ))
                .into());
            }
        }

        let required_permissions = upsert_permissions(&claims_context.claims.sub, &events);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        let events = self
            .event_store
            .upsert(events)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;
        Ok(Response::new(UpsertEventsResponse { events }))
    }

    async fn query_events(
        &self,
        request: Request<QueryEventsRequest>,
    ) -> Result<Response<QueryEventsResponse>, Status> {
        let (_, extensions, query_request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let query = query_request.query.map(try_parse_event_query).transpose()?;

        let events = self
            .event_store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        // Check permissions for all events returned by the query
        let required_permissions = query_permissions(&claims_context.claims.sub, &events);

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

        // Filter events to only include those the user has permission to view
        let filtered_events = events
            .into_iter()
            .filter(|event| !hidden_events.contains(&event.id))
            .collect::<Vec<_>>();

        Ok(Response::new(QueryEventsResponse {
            events: filtered_events,
        }))
    }

    async fn delete_events(
        &self,
        request: Request<proto::DeleteEventsRequest>,
    ) -> Result<Response<DeleteEventsResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let event_ids = request.ids;

        // Check if user has EventAdmin permission for all events to be deleted
        let required_permissions = delete_permissions(&claims_context.claims.sub, &event_ids);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        self.event_store
            .delete(&event_ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteEventsResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use super::Service;
    use crate::{
        api::middleware::authentication::ClaimsContext,
        authentication::Claims,
        proto::{
            event_service_server::EventService as _, permission_role, Event, EventRole,
            OrganizationRole, Permission, PermissionRole, QueryEventsRequest, QueryEventsResponse,
            UpsertEventsRequest, UpsertEventsResponse,
        },
        store::{event::MockStore as MockEventStore, permission::MockStore as MockPermissionStore},
        test_helpers::StatusCompare,
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
            result: Result<UpsertEventsResponse, Status>,
        }

        let new_id = "id";
        let user_id = "user_id";
        let org_id = "org_id";

        let event = Event {
            id: "".to_string(),
            organization_id: org_id.to_string(),
            name: "Test Event".to_string(),
        };

        let mut returned_event = event.clone();
        returned_event.id = new_id.to_string();

        let tc = match test_name {
            InsertTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(UpsertEventsResponse {
                    events: vec![returned_event.clone()],
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

        let mut event_store = MockEventStore::new();
        let mut permission_store = MockPermissionStore::new();

        event_store
            .expect_upsert()
            .with(eq(vec![event.clone()]))
            .returning(move |_| {
                let returned_event = returned_event.clone();
                Box::pin(async move { Ok(vec![returned_event]) })
            });

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

        let service = Service::new(Arc::new(event_store), Arc::new(permission_store));

        let mut request = Request::new(UpsertEventsRequest {
            events: vec![event],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.upsert_events(request).await.map(|r| r.into_inner());

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
        let id = "event_id";
        let user_id = "user_id";

        let event = Event {
            id: id.to_string(),
            organization_id: "org_id".to_string(),
            name: "Test Event".to_string(),
        };

        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Vec<Event>,
        }

        let tc = match test_name {
            QueryTest::Success => TestCase {
                missing_permissions: vec![],
                result: vec![event.clone()],
            },
            QueryTest::Filtered => TestCase {
                missing_permissions: vec![Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: id.to_string(),
                        })),
                    }),
                }],
                result: vec![],
            },
        };

        let mut permission_store = MockPermissionStore::new();
        let mut event_store = MockEventStore::new();

        event_store.expect_query().returning(move |_| {
            let event = event.clone();
            Box::pin(async move { Ok(vec![event]) })
        });

        permission_store
            .expect_permission_check()
            .with(eq(vec![Permission {
                id: "".to_string(),
                user_id: user_id.to_string(),
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::EventViewer(EventRole {
                        event_id: id.to_string(),
                    })),
                }),
            }]))
            .returning(move |_| {
                let missing_permissions = tc.missing_permissions.clone();
                Box::pin(async move { Ok(missing_permissions) })
            });

        let service = Service::new(Arc::new(event_store), Arc::new(permission_store));

        let mut request = Request::new(QueryEventsRequest { query: None });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.query_events(request).await.unwrap();

        assert_eq!(response.into_inner().events, tc.result);
    }
}
