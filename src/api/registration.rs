use crate::{
    api::{common::try_logical_string_query, store_error_to_status, ValidationError},
    proto::{
        self, compound_registration_query, permission_role, registration_query,
        DeleteRegistrationsRequest, DeleteRegistrationsResponse, EventRole, Permission,
        PermissionRole, QueryRegistrationsRequest, QueryRegistrationsResponse, Registration,
        RegistrationQuery, UpsertRegistrationsRequest, UpsertRegistrationsResponse,
    },
    store::{
        registration::{Query, Store},
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

fn upsert_permissions(user_id: &str, schemas: &[Registration]) -> Vec<Permission> {
    schemas
        .iter()
        .flat_map(|schema| {
            vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: schema.event_id.clone(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: schema.event_id.clone(),
                        })),
                    }),
                },
            ]
        })
        .collect::<Vec<_>>()
}

fn query_permissions(user_id: &str, schemas: &[Registration]) -> Vec<Permission> {
    schemas
        .iter()
        .map(|schema| Permission {
            id: "".to_string(),
            user_id: user_id.to_string(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventViewer(EventRole {
                    event_id: schema.event_id.clone(),
                })),
            }),
        })
        .collect::<Vec<_>>()
}

fn delete_permissions(user_id: &str, schema_ids: &[String]) -> Vec<Permission> {
    schema_ids
        .iter()
        .flat_map(|schema_id| {
            vec![
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventEditor(EventRole {
                            event_id: schema_id.clone(),
                        })),
                    }),
                },
                Permission {
                    id: "".to_string(),
                    user_id: user_id.to_string(),
                    role: Some(PermissionRole {
                        role: Some(permission_role::Role::EventViewer(EventRole {
                            event_id: schema_id.clone(),
                        })),
                    }),
                },
            ]
        })
        .collect::<Vec<_>>()
}

#[tonic::async_trait]
impl<StoreType: Store> proto::registration_service_server::RegistrationService
    for Service<StoreType>
{
    async fn upsert_registrations(
        &self,
        request: Request<UpsertRegistrationsRequest>,
    ) -> Result<Response<UpsertRegistrationsResponse>, Status> {
        let request_registrations = request.into_inner().registrations;

        for (idx, registration) in request_registrations.iter().enumerate() {
            validate_registration(registration).map_err(|e| -> Status {
                e.with_context(&format!("registrations[{}]", idx)).into()
            })?;
        }

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
        let query = request.into_inner().query;

        let query = query
            .map(|query| try_parse_registration_query(query))
            .transpose()?;

        let registrations = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(QueryRegistrationsResponse { registrations }))
    }

    async fn delete_registrations(
        &self,
        request: Request<DeleteRegistrationsRequest>,
    ) -> Result<Response<DeleteRegistrationsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteRegistrationsResponse {}))
    }
}
