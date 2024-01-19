use tonic::{Code, Status};

use crate::store::Error;

pub mod event;
pub mod registration;
pub mod registration_schema;

fn store_error_to_status(err: Error) -> Status {
    let code = match err {
        Error::IdDoesNotExist(_) | Error::SomeIdDoesNotExist | Error::UnknownEnum => Code::NotFound,
        Error::InsertionError(_)
        | Error::FetchError(_)
        | Error::UpdateError(_)
        | Error::DeleteError(_)
        | Error::CheckExistsError(_)
        | Error::TransactionStartError(_)
        | Error::TransactionFailed(_)
        | Error::TooManyItems(_)
        | Error::ColumnParseError(_) => Code::Internal,
    };

    Status::new(code, format!("{}", err))
}
