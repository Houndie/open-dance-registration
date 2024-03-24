use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::store::{
    registration_schema::{Query, Store},
    CompoundOperator, CompoundQuery,
};
use common::proto::{
    self, compound_registration_schema_query, multi_select_type, registration_schema_item_type,
    registration_schema_query, select_type, text_type, DeleteRegistrationSchemasResponse,
    QueryRegistrationSchemasRequest, QueryRegistrationSchemasResponse, RegistrationSchema,
    RegistrationSchemaItem, RegistrationSchemaQuery, UpsertRegistrationSchemasRequest,
    UpsertRegistrationSchemasResponse,
};

use super::{common::try_logical_string_query, ValidationError};

#[derive(Debug)]
pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
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

impl TryFrom<RegistrationSchemaQuery> for Query {
    type Error = ValidationError;

    fn try_from(query: RegistrationSchemaQuery) -> Result<Self, Self::Error> {
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
                    Err(_) => {
                        return Err(ValidationError::new_invalid_enum("query.compound.operator"))
                    }
                };

                let queries = compound_query
                    .queries
                    .into_iter()
                    .enumerate()
                    .map(|(idx, query)| {
                        query.try_into().map_err(|e: Self::Error| {
                            e.with_context(&format!("query.compound.queries[{}]", idx))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Query::Compound(CompoundQuery { operator, queries }))
            }
            None => Err(ValidationError::new_empty("query")),
        }
    }
}

#[tonic::async_trait]
impl<StoreType: Store> proto::registration_schema_service_server::RegistrationSchemaService
    for Service<StoreType>
{
    async fn upsert_registration_schemas(
        &self,
        request: Request<UpsertRegistrationSchemasRequest>,
    ) -> Result<Response<UpsertRegistrationSchemasResponse>, Status> {
        let request_schemas = request.into_inner().registration_schemas;

        for (idx, schema) in request_schemas.iter().enumerate() {
            validate_registration_schema(schema).map_err(|e| -> Status {
                e.with_context(&format!("registration_schemas[{}]", idx))
                    .into()
            })?;
        }

        let registration_schemas = self
            .store
            .upsert(request_schemas)
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(UpsertRegistrationSchemasResponse {
            registration_schemas,
        }))
    }

    async fn query_registration_schemas(
        &self,
        request: Request<QueryRegistrationSchemasRequest>,
    ) -> Result<Response<QueryRegistrationSchemasResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .map(|q| -> Result<_, ValidationError> { q.try_into() })
            .transpose()?;

        let registration_schemas = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { e.into() })?;
        Ok(Response::new(QueryRegistrationSchemasResponse {
            registration_schemas,
        }))
    }

    async fn delete_registration_schemas(
        &self,
        request: Request<proto::DeleteRegistrationSchemasRequest>,
    ) -> Result<Response<DeleteRegistrationSchemasResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(DeleteRegistrationSchemasResponse {}))
    }
}
