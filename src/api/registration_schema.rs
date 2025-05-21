use crate::{
    api::{
        authorization_state_to_status, common::try_logical_string_query,
        err_missing_claims_context, middleware::authentication::ClaimsContext,
        store_error_to_status, ValidationError,
    },
    proto::{
        self, compound_registration_schema_query, multi_select_type, permission_role,
        registration_schema_item_type, registration_schema_query, select_type, text_type,
        DeleteRegistrationSchemasResponse, EventRole, Permission, PermissionRole,
        QueryRegistrationSchemasRequest, QueryRegistrationSchemasResponse, RegistrationSchema,
        RegistrationSchemaItem, RegistrationSchemaQuery, UpsertRegistrationSchemasRequest,
        UpsertRegistrationSchemasResponse,
    },
    store::{
        permission::Store as PermissionStore,
        registration_schema::{Query, Store},
        CompoundOperator, CompoundQuery,
    },
};
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};

#[derive(Debug)]
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

fn validate_registration_schema_item(item: &RegistrationSchemaItem) -> Result<(), ValidationError> {
    if item.name.is_empty() {
        return Err(ValidationError::new_empty("name"));
    }

    let outer_type = match &item.r#type {
        Some(t) => t,
        None => return Err(ValidationError::new_empty("type")),
    };

    let typ = match &outer_type.r#type {
        Some(t) => t,
        None => return Err(ValidationError::new_empty("type.type")),
    };

    match typ {
        registration_schema_item_type::Type::Text(text) => {
            if text_type::Display::try_from(text.display).is_err() {
                return Err(ValidationError::new_invalid_enum("type.text.display"));
            }
        }

        registration_schema_item_type::Type::Checkbox(_) => (),

        registration_schema_item_type::Type::Select(select) => {
            if select_type::Display::try_from(select.display).is_err() {
                return Err(ValidationError::new_invalid_enum("type.select.display"));
            }

            if select.options.len() > i32::MAX as usize {
                return Err(ValidationError::new_too_many_items("type.select.options"));
            }
        }

        registration_schema_item_type::Type::MultiSelect(multi_select) => {
            if multi_select_type::Display::try_from(multi_select.display).is_err() {
                return Err(ValidationError::new_invalid_enum(
                    "type.multi_select.display",
                ));
            }

            if multi_select.options.len() > i32::MAX as usize {
                return Err(ValidationError::new_too_many_items(
                    "type.multi_select.options",
                ));
            }
        }
    };

    Ok(())
}

fn validate_registration_schema(
    registration_schema: &RegistrationSchema,
) -> Result<(), ValidationError> {
    if registration_schema.event_id.is_empty() {
        return Err(ValidationError::new_empty("event_id"));
    }

    if registration_schema.items.len() > i32::MAX as usize {
        return Err(ValidationError::new_too_many_items("items"));
    }

    for (idx, item) in registration_schema.items.iter().enumerate() {
        validate_registration_schema_item(item)
            .map_err(|e| e.with_context(&format!("items[{}]", idx)))?;
    }

    Ok(())
}

fn try_parse_registration_schema_query(
    query: RegistrationSchemaQuery,
) -> Result<Query, ValidationError> {
    match query.query {
        Some(registration_schema_query::Query::EventId(event_id_query)) => Ok(Query::EventId(
            try_logical_string_query(event_id_query)
                .map_err(|e| e.with_context("query.event_id"))?,
        )),

        Some(registration_schema_query::Query::Compound(compound_query)) => {
            let operator = match compound_registration_schema_query::Operator::try_from(
                compound_query.operator,
            ) {
                Ok(compound_registration_schema_query::Operator::And) => CompoundOperator::And,
                Ok(compound_registration_schema_query::Operator::Or) => CompoundOperator::Or,
                Err(_) => return Err(ValidationError::new_invalid_enum("query.compound.operator")),
            };

            let queries = compound_query
                .queries
                .into_iter()
                .enumerate()
                .map(|(idx, query)| {
                    try_parse_registration_schema_query(query).map_err(|e: ValidationError| {
                        e.with_context(&format!("query.compound.queries[{}]", idx))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Query::Compound(CompoundQuery { operator, queries }))
        }
        None => Err(ValidationError::new_empty("query")),
    }
}

fn upsert_permissions(user_id: &str, schemas: &[RegistrationSchema]) -> Vec<Permission> {
    schemas
        .iter()
        .map(|schema| {
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
        .flatten()
        .collect::<Vec<_>>()
}

fn query_permissions(user_id: &str, schemas: &[RegistrationSchema]) -> Vec<Permission> {
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

#[tonic::async_trait]
impl<StoreType: Store, PermissionStoreType: PermissionStore>
    proto::registration_schema_service_server::RegistrationSchemaService
    for Service<StoreType, PermissionStoreType>
{
    async fn upsert_registration_schemas(
        &self,
        request: Request<UpsertRegistrationSchemasRequest>,
    ) -> Result<Response<UpsertRegistrationSchemasResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let request_schemas = request.registration_schemas;

        for (idx, schema) in request_schemas.iter().enumerate() {
            validate_registration_schema(schema).map_err(|e| -> Status {
                e.with_context(&format!("registration_schemas[{}]", idx))
                    .into()
            })?;
        }

        let required_permissions = upsert_permissions(&claims_context.claims.sub, &request_schemas);

        let failed_permissions = self
            .permission_store
            .permission_check(required_permissions)
            .await
            .map_err(store_error_to_status)?;

        authorization_state_to_status(failed_permissions)?;

        let registration_schemas = self
            .store
            .upsert(request_schemas)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(UpsertRegistrationSchemasResponse {
            registration_schemas,
        }))
    }

    async fn query_registration_schemas(
        &self,
        request: Request<QueryRegistrationSchemasRequest>,
    ) -> Result<Response<QueryRegistrationSchemasResponse>, Status> {
        let (_, extensions, query_request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let query = query_request
            .query
            .map(|q| -> Result<_, ValidationError> { try_parse_registration_schema_query(q) })
            .transpose()?;

        let registration_schemas = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;
            
        // Check permissions for all schemas returned by the query
        let required_permissions = query_permissions(&claims_context.claims.sub, &registration_schemas);

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

        // Filter schemas to only include those the user has permission to view
        let filtered_schemas = registration_schemas
            .into_iter()
            .filter(|schema| !hidden_events.contains(&schema.event_id))
            .collect::<Vec<_>>();

        Ok(Response::new(QueryRegistrationSchemasResponse {
            registration_schemas: filtered_schemas,
        }))
    }

    async fn delete_registration_schemas(
        &self,
        request: Request<proto::DeleteRegistrationSchemasRequest>,
    ) -> Result<Response<DeleteRegistrationSchemasResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteRegistrationSchemasResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use super::Service;
    use crate::{
        api::middleware::authentication::ClaimsContext,
        authentication::Claims,
        proto::{
            permission_role, registration_schema_item_type, registration_schema_service_server::RegistrationSchemaService,
            text_type, EventRole, Permission, PermissionRole, QueryRegistrationSchemasRequest,
            RegistrationSchema, RegistrationSchemaItem, RegistrationSchemaItemType, TextType,
            UpsertRegistrationSchemasRequest, UpsertRegistrationSchemasResponse,
        },
        store::{
            permission::MockStore as MockPermissionStore,
            registration_schema::MockStore as MockRegistrationSchemaStore,
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
            result: Result<UpsertRegistrationSchemasResponse, Status>,
        }

        let item_id = "item_id";
        let user_id = "user_id";
        let event_id = "event_id";

        let text_type = TextType {
            default: "".to_string(),
            display: text_type::Display::Small as i32,
        };

        let schema_item = RegistrationSchemaItem {
            id: item_id.to_string(),
            name: "Test field".to_string(),
            r#type: Some(RegistrationSchemaItemType {
                r#type: Some(registration_schema_item_type::Type::Text(text_type)),
            }),
        };

        let schema = RegistrationSchema {
            event_id: event_id.to_string(),
            items: vec![schema_item],
        };

        let tc = match test_name {
            UpsertTest::Success => TestCase {
                missing_permissions: vec![],
                result: Ok(UpsertRegistrationSchemasResponse {
                    registration_schemas: vec![schema.clone()],
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

        let mut schema_store = MockRegistrationSchemaStore::new();
        let mut permission_store = MockPermissionStore::new();

        {
            let schema = schema.clone();
            schema_store
                .expect_upsert()
                .with(eq(vec![schema.clone()]))
                .returning(move |_| {
                    let schema = schema.clone();
                    Box::pin(async move { Ok(vec![schema]) })
                });
        }

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

        let service = Service::new(Arc::new(schema_store), Arc::new(permission_store));

        let mut request = Request::new(UpsertRegistrationSchemasRequest {
            registration_schemas: vec![schema],
        });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service
            .upsert_registration_schemas(request)
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
        let event_id = "event_id";
        let user_id = "user_id";
        let item_id = "item_id";

        let text_type = TextType {
            default: "".to_string(),
            display: text_type::Display::Small as i32,
        };

        let schema_item = RegistrationSchemaItem {
            id: item_id.to_string(),
            name: "Test field".to_string(),
            r#type: Some(RegistrationSchemaItemType {
                r#type: Some(registration_schema_item_type::Type::Text(text_type)),
            }),
        };

        let schema = RegistrationSchema {
            event_id: event_id.to_string(),
            items: vec![schema_item],
        };

        struct TestCase {
            missing_permissions: Vec<Permission>,
            result: Vec<RegistrationSchema>,
        }

        let tc = match test_name {
            QueryTest::Success => TestCase {
                missing_permissions: vec![],
                result: vec![schema.clone()],
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
        let mut schema_store = MockRegistrationSchemaStore::new();

        schema_store.expect_query().returning(move |_| {
            let schema = schema.clone();
            Box::pin(async move { Ok(vec![schema]) })
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

        let service = Service::new(Arc::new(schema_store), Arc::new(permission_store));

        let mut request = Request::new(QueryRegistrationSchemasRequest { query: None });

        request.extensions_mut().insert(ClaimsContext {
            claims: Claims {
                sub: user_id.to_string(),
                ..Default::default()
            },
        });

        let response = service.query_registration_schemas(request).await.unwrap();

        assert_eq!(response.into_inner().registration_schemas, tc.result);
    }
}
