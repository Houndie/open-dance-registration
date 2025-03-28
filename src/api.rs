use crate::{
    proto::{permission_role, Permission},
    store,
};
use std::fmt::{Display, Formatter};
use thiserror::Error as ThisError;
use tonic::{Code, Status};

pub mod authentication;
mod common;
pub mod event;
pub mod middleware;
pub mod organization;
pub mod permission;
pub mod registration;
pub mod registration_schema;
pub mod user;

fn store_error_to_status(err: store::Error) -> Status {
    let code = match err {
        store::Error::IdDoesNotExist(_) => Code::NotFound,
        store::Error::InsertionError(_)
        | store::Error::FetchError(_)
        | store::Error::UpdateError(_)
        | store::Error::DeleteError(_)
        | store::Error::CheckExistsError(_)
        | store::Error::TransactionStartError(_)
        | store::Error::TransactionFailed(_)
        | store::Error::ColumnParseError(_) => Code::Internal,
    };

    Status::new(code, format!("{}", err))
}

fn authorization_state_to_status(mut failed_permissions: Vec<Permission>) -> Result<(), Status> {
    match failed_permissions.pop() {
        Some(p) => match p.role.unwrap().role.unwrap() {
            permission_role::Role::ServerAdmin(_) => Err(Status::permission_denied("")),
            permission_role::Role::OrganizationAdmin(o) => {
                if failed_permissions.iter().any(|p| {
                    matches!(
                        p.role.as_ref().unwrap().role.as_ref().unwrap(),
                        permission_role::Role::OrganizationViewer(oo) if oo.organization_id == o.organization_id
                    )
                }) {
                    Err(Status::not_found(o.organization_id))
                } else {
                    Err(Status::permission_denied(""))
                }
            }
            permission_role::Role::OrganizationViewer(o) => {
                Err(Status::not_found(o.organization_id))
            }
            permission_role::Role::EventAdmin(e)
            | permission_role::Role::EventEditor(e) => {
                if failed_permissions.iter().any(|p| {
                    matches!(
                        p.role.as_ref().unwrap().role.as_ref().unwrap(),
                        permission_role::Role::EventViewer(ee) if ee.event_id == e.event_id
                    )
                }) {
                    Err(Status::not_found(e.event_id))
                } else {
                    Err(Status::permission_denied(""))
                }
            }
            permission_role::Role::EventViewer(e) => Err(Status::not_found(e.event_id)),
        },
        None => Ok(()),
    }
}

fn err_missing_claims_context() -> Status {
    Status::internal("Missing claims context")
}

#[derive(Debug)]
enum ValidationErrorReason {
    EmptyField,
    TooManyItems,
    InvalidEnum,
    InvalidValue,
}

impl Display for ValidationErrorReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationErrorReason::EmptyField => write!(f, "cannot be empty"),
            ValidationErrorReason::TooManyItems => write!(f, "contains too many items"),
            ValidationErrorReason::InvalidEnum => write!(f, "contains invalid enum value"),
            ValidationErrorReason::InvalidValue => write!(f, "contains an invalid value"),
        }
    }
}

#[derive(Debug, ThisError)]
#[error("{field} {reason}")]
pub struct ValidationError {
    field: String,
    reason: ValidationErrorReason,
}

impl ValidationError {
    fn new(field: &str, reason: ValidationErrorReason) -> Self {
        ValidationError {
            field: field.to_string(),
            reason,
        }
    }

    fn new_empty(field: &str) -> Self {
        ValidationError::new(field, ValidationErrorReason::EmptyField)
    }

    fn new_too_many_items(field: &str) -> Self {
        ValidationError::new(field, ValidationErrorReason::TooManyItems)
    }

    fn new_invalid_enum(field: &str) -> Self {
        ValidationError::new(field, ValidationErrorReason::InvalidEnum)
    }

    fn new_invalid_value(field: &str) -> Self {
        ValidationError::new(field, ValidationErrorReason::InvalidValue)
    }

    fn with_context(self, context: &str) -> Self {
        let field = if self.field == "" || self.field.starts_with('[') {
            format!("{}{}", context, self.field)
        } else {
            format!("{}.{}", context, self.field)
        };

        ValidationError {
            field,
            reason: self.reason,
        }
    }
}

impl From<ValidationError> for Status {
    fn from(err: ValidationError) -> Self {
        Status::invalid_argument(err.to_string())
    }
}
