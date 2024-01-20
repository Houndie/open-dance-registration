use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::store::registration_schema::Store;
use common::proto::{
    self, multi_select_type, registration_schema_item_type, select_type, text_type,
    DeleteRegistrationSchemasResponse, ListRegistrationSchemasRequest,
    ListRegistrationSchemasResponse, RegistrationSchema, RegistrationSchemaItem,
    UpsertRegistrationSchemasRequest, UpsertRegistrationSchemasResponse,
};

use super::{store_error_to_status, ValidationError};

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
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(UpsertRegistrationSchemasResponse {
            registration_schemas,
        }))
    }

    async fn list_registration_schemas(
        &self,
        request: Request<ListRegistrationSchemasRequest>,
    ) -> Result<Response<ListRegistrationSchemasResponse>, Status> {
        let registration_schemas = self
            .store
            .list(request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(ListRegistrationSchemasResponse {
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
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteRegistrationSchemasResponse {}))
    }
}
