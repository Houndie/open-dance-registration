use std::fmt::{Display, Formatter};

use tonic::{Code, Status};

use crate::store;
use thiserror::Error as ThisError;

pub mod authentication;
mod common;
pub mod event;
pub mod organization;
pub mod registration;
pub mod registration_schema;
pub mod user;

impl From<store::Error> for Status {
    fn from(err: store::Error) -> Self {
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
