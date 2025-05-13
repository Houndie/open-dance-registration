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
