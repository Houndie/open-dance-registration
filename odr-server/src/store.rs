use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

mod common;
pub mod event;
pub mod keys;
pub mod organization;
pub mod registration;
pub mod registration_schema;
pub mod user;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("id {0} does not exist")]
    IdDoesNotExist(String),

    #[error("error inserting new event into data store: {0}")]
    InsertionError(#[source] sqlx::Error),

    #[error("error fetching event from database: {0}")]
    FetchError(#[source] sqlx::Error),

    #[error("error deleting event from database: {0}")]
    DeleteError(#[source] sqlx::Error),

    #[error("error checking event existance in database: {0}")]
    CheckExistsError(#[source] sqlx::Error),

    #[error("error updating event: {0}")]
    UpdateError(#[source] sqlx::Error),

    #[error("transaction failed to commit: {0}")]
    TransactionFailed(#[source] sqlx::Error),

    #[error("transaction failed to start: {0}")]
    TransactionStartError(#[source] sqlx::Error),

    #[error("unable to parse column {0}")]
    ColumnParseError(&'static str),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self {
            Self::IdDoesNotExist(_) => StatusCode::NOT_FOUND,
            Self::InsertionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FetchError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DeleteError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::CheckExistsError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UpdateError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TransactionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TransactionStartError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ColumnParseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}
