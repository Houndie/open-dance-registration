use std::fmt::{Display, Formatter};

use tonic::{Code, Status};

use crate::store::Error;
use thiserror::Error as ThisError;

pub mod authentication;
mod common;
pub mod event;
pub mod organization;
pub mod registration;
pub mod registration_schema;
pub mod user;

fn store_error_to_status(err: Error) -> Status {
    let code = match err {
        Error::IdDoesNotExist(_) => Code::NotFound,
        Error::InsertionError(_)
        | Error::FetchError(_)
        | Error::UpdateError(_)
        | Error::DeleteError(_)
        | Error::CheckExistsError(_)
        | Error::TransactionStartError(_)
        | Error::TransactionFailed(_)
        | Error::ColumnParseError(_) => Code::Internal,
    };

    Status::new(code, format!("{}", err))
}

#[derive(Debug)]
enum ValidationErrorReason {
    EmptyField,
    TooManyItems,
    InvalidEnum,
}

impl Display for ValidationErrorReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationErrorReason::EmptyField => write!(f, "cannot be empty"),
            ValidationErrorReason::TooManyItems => write!(f, "contains too many items"),
            ValidationErrorReason::InvalidEnum => write!(f, "contains invalid enum value"),
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
