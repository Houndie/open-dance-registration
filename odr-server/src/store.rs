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

pub trait Queryable {
    fn where_clause(&self) -> String;
}

pub trait Bindable<'q, DB: sqlx::Database> {
    fn bind<O>(
        &'q self,
        query_builder: sqlx::query::QueryAs<
            'q,
            DB,
            O,
            <DB as sqlx::database::HasArguments<'q>>::Arguments,
        >,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::database::HasArguments<'q>>::Arguments>;
}

pub enum CompoundOperator {
    And,
    Or,
}

pub struct CompoundQuery<Q: Queryable> {
    pub operator: CompoundOperator,
    pub queries: Vec<Q>,
}

impl<Q: Queryable> Queryable for CompoundQuery<Q> {
    fn where_clause(&self) -> String {
        let operator = match self.operator {
            CompoundOperator::And => " AND ",
            CompoundOperator::Or => " OR ",
        };

        let where_clauses = itertools::Itertools::intersperse(
            self.queries.iter().map(|query| query.where_clause()),
            operator.to_owned(),
        )
        .collect::<String>();

        format!("({})", where_clauses)
    }
}

impl<'q, DB: sqlx::Database, Q: Queryable + Bindable<'q, DB>> Bindable<'q, DB>
    for CompoundQuery<Q>
{
    fn bind<O>(
        &'q self,
        query_builder: sqlx::query::QueryAs<
            'q,
            DB,
            O,
            <DB as sqlx::database::HasArguments<'q>>::Arguments,
        >,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::database::HasArguments<'q>>::Arguments> {
        self.queries
            .iter()
            .fold(query_builder, |query_builder, query| {
                query.bind(query_builder)
            })
    }
}

pub trait Field {
    type Item;
    fn field() -> &'static str;
}

pub enum LogicalQuery<F: Field> {
    Equals(F::Item),
    NotEquals(F::Item),
}

impl<F: Field> Queryable for LogicalQuery<F> {
    fn where_clause(&self) -> String {
        match self {
            LogicalQuery::Equals(_) => format!("{} = ?", F::field()),
            LogicalQuery::NotEquals(_) => format!("{} != ?", F::field()),
        }
    }
}

impl<'q, DB: sqlx::Database, F: Field> Bindable<'q, DB> for LogicalQuery<F>
where
    F::Item: sqlx::Encode<'q, DB> + sqlx::Type<DB> + Sync,
{
    fn bind<O>(
        &'q self,
        query_builder: sqlx::query::QueryAs<
            'q,
            DB,
            O,
            <DB as sqlx::database::HasArguments<'q>>::Arguments,
        >,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::database::HasArguments<'q>>::Arguments> {
        match self {
            LogicalQuery::Equals(value) => query_builder.bind(value),
            LogicalQuery::NotEquals(value) => query_builder.bind(value),
        }
    }
}
