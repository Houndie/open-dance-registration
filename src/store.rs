mod common;
pub mod event;
pub mod keys;
pub mod organization;
pub mod permission;
pub mod registration;
pub mod registration_schema;
pub mod user;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("id {0} does not exist")]
    IdDoesNotExist(String),

    #[error("error inserting new item into data store: {0}")]
    InsertionError(#[source] sqlx::Error),

    #[error("error fetching item from database: {0}")]
    FetchError(#[source] sqlx::Error),

    #[error("error deleting item from database: {0}")]
    DeleteError(#[source] sqlx::Error),

    #[error("error checking item existance in database: {0}")]
    CheckExistsError(#[source] sqlx::Error),

    #[error("error updating item: {0}")]
    UpdateError(#[source] sqlx::Error),

    #[error("transaction failed to commit: {0}")]
    TransactionFailed(#[source] sqlx::Error),

    #[error("transaction failed to start: {0}")]
    TransactionStartError(#[source] sqlx::Error),

    #[error("unable to parse column {0}")]
    ColumnParseError(&'static str),
}

pub trait Queryable {
    fn where_clause(&self) -> String;
}

pub trait Bindable<'q, DB: sqlx::Database> {
    fn bind<O>(
        &'q self,
        query_builder: sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>>,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>>;
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

        itertools::Itertools::intersperse(
            self.queries
                .iter()
                .map(|query| format!("({})", query.where_clause())),
            operator.to_owned(),
        )
        .collect::<String>()
    }
}

impl<'q, DB: sqlx::Database, Q: Queryable + Bindable<'q, DB>> Bindable<'q, DB>
    for CompoundQuery<Q>
{
    fn bind<O>(
        &'q self,
        query_builder: sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>>,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>> {
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
        query_builder: sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>>,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>> {
        match self {
            LogicalQuery::Equals(value) => query_builder.bind(value),
            LogicalQuery::NotEquals(value) => query_builder.bind(value),
        }
    }
}
