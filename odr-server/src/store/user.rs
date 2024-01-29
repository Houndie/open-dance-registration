use std::sync::Arc;

use argon2::password_hash::PasswordHashString;
use sqlx::SqlitePool;

use super::{
    common::{ids_in_table, new_id},
    Bindable as _, Error, Queryable as _,
};

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    email: String,
    password: Option<String>,
    display_name: String,
}

impl TryFrom<UserRow> for User {
    type Error = Error;

    fn try_from(row: UserRow) -> Result<Self, Error> {
        println!("A1 {:?}", row.password);
        let password = match row.password {
            Some(password) => PasswordType::Set(
                PasswordHashString::new(&password)
                    .map_err(|_| Error::ColumnParseError("password"))?,
            ),
            None => PasswordType::Unset,
        };

        Ok(User {
            id: row.id,
            email: row.email,
            password,
            display_name: row.display_name,
        })
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub enum PasswordType {
    Set(PasswordHashString),
    Unset,

    #[default]
    Unchanged,
}

impl PasswordType {
    pub fn is_unchanged(&self) -> bool {
        matches!(self, PasswordType::Unchanged)
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password: PasswordType,
    pub display_name: String,
}

pub struct IdField;

impl super::Field for IdField {
    type Item = String;

    fn field() -> &'static str {
        "id"
    }
}

pub type IdQuery = super::LogicalQuery<IdField>;

pub struct EmailField;

impl super::Field for EmailField {
    type Item = String;

    fn field() -> &'static str {
        "email"
    }
}

pub type EmailQuery = super::LogicalQuery<EmailField>;

pub enum Query {
    Id(IdQuery),
    Email(EmailQuery),
    PasswordIsSet(bool),
    CompoundQuery(super::CompoundQuery<Query>),
}

impl super::Queryable for Query {
    fn where_clause(&self) -> String {
        match self {
            Query::Id(q) => q.where_clause(),
            Query::Email(q) => q.where_clause(),
            Query::PasswordIsSet(true) => "password IS NOT NULL".to_owned(),
            Query::PasswordIsSet(false) => "password IS NULL".to_owned(),
            Query::CompoundQuery(compound_query) => compound_query.where_clause(),
        }
    }
}

impl<'q, DB: sqlx::Database> super::Bindable<'q, DB> for Query
where
    <EmailField as super::Field>::Item: sqlx::Type<DB> + sqlx::Encode<'q, DB>,
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
            Query::Id(q) => q.bind(query_builder),
            Query::Email(q) => q.bind(query_builder),
            Query::PasswordIsSet(_) => query_builder,
            Query::CompoundQuery(compound_query) => compound_query.bind(query_builder),
        }
    }
}

type QueryBuilder<'q> = sqlx::query::Query<
    'q,
    sqlx::Sqlite,
    <sqlx::Sqlite as sqlx::database::HasArguments<'q>>::Arguments,
>;

fn bind_user<'q>(query_builder: QueryBuilder<'q>, user: &'q User) -> QueryBuilder<'q> {
    let query_builder = query_builder.bind(&user.id).bind(&user.email);

    let query_builder = match &user.password {
        PasswordType::Set(password) => query_builder.bind(Some(password.as_str())),
        PasswordType::Unset => query_builder.bind(None as Option<&str>),
        PasswordType::Unchanged => query_builder,
    };

    query_builder.bind(&user.display_name)
}

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn upsert(&self, users: Vec<User>) -> Result<Vec<User>, Error>;
    async fn query(&self, query: Option<&Query>) -> Result<Vec<User>, Error>;
    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error>;
}

#[derive(Debug)]
pub struct SqliteStore {
    pool: Arc<SqlitePool>,
}

impl SqliteStore {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        SqliteStore { pool }
    }
}

#[tonic::async_trait]
impl Store for SqliteStore {
    async fn upsert(&self, users: Vec<User>) -> Result<Vec<User>, Error> {
        let (inserts, updates): (Vec<_>, Vec<_>) = users
            .into_iter()
            .enumerate()
            .partition(|(_, user)| user.id == "");

        if !updates.is_empty() {
            // Make sure events exist
            ids_in_table(
                &*self.pool,
                "users",
                updates.iter().map(|(_, user)| user.id.as_str()),
            )
            .await?;
        }

        let (updates_without_password, updates_with_password): (Vec<_>, Vec<_>) = updates
            .into_iter()
            .partition(|(_, user)| user.password.is_unchanged());

        let inserts = inserts
            .into_iter()
            .map(|(idx, mut user)| {
                user.id = new_id();
                (idx, user)
            })
            .collect::<Vec<_>>();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Error::TransactionStartError(e))?;

        if !inserts.is_empty() {
            let values_clause = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?, ?, ?)").take(inserts.len()),
                " , ",
            )
            .collect::<String>();

            let query = format!(
                "INSERT INTO users (id, email, password, display_name) VALUES {}",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = inserts
                .iter()
                .fold(query_builder, |query_builder, (_, user)| {
                    bind_user(query_builder, user)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::InsertionError(e))?;
        }

        if !updates_with_password.is_empty() {
            let values_clause = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?, ?, ?)").take(updates_with_password.len()),
                " , ",
            )
            .collect::<String>();

            let query = format!(
                "WITH mydata(id, email, password, display_name) AS (VALUES {}) 
                UPDATE users 
                SET email = mydata.email, 
                    password = mydata.password, 
                    display_name = mydata.display_name
                FROM mydata
                WHERE users.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = updates_with_password
                .iter()
                .fold(query_builder, |query_builder, (_, user)| {
                    bind_user(query_builder, user)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;
        }

        if !updates_without_password.is_empty() {
            let values_clause = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?, ?)").take(updates_without_password.len()),
                " , ",
            )
            .collect::<String>();

            let query = format!(
                "WITH mydata(id, email, display_name) AS (VALUES {}) 
                UPDATE users 
                SET email = mydata.email, 
                    display_name = mydata.display_name
                FROM mydata
                WHERE users.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = updates_without_password
                .iter()
                .fold(query_builder, |query_builder, (_, user)| {
                    bind_user(query_builder, user)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;
        }

        tx.commit().await.map_err(|e| Error::TransactionFailed(e))?;

        let mut outputs = Vec::new();
        outputs.resize(
            inserts.len() + updates_with_password.len() + updates_without_password.len(),
            User::default(),
        );

        inserts
            .into_iter()
            .chain(updates_with_password)
            .chain(updates_without_password)
            .for_each(|(idx, user)| {
                outputs[idx] = user;
            });

        Ok(outputs)
    }

    async fn query(&self, query: Option<&Query>) -> Result<Vec<User>, Error> {
        let base_query_string = "SELECT id, email, password, display_name FROM users";
        let query_string = match query {
            Some(query) => format!("{} WHERE {}", base_query_string, query.where_clause()),
            None => base_query_string.to_owned(),
        };

        let query_builder = sqlx::query_as(&query_string);
        let query_builder = match query {
            Some(query) => query.bind(query_builder),
            None => query_builder,
        };

        let rows: Vec<UserRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| Error::FetchError(e))?;

        let users = rows
            .into_iter()
            .map(|row| row.try_into())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(users)
    }

    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }

        ids_in_table(&*self.pool, "users", ids.iter().map(|id| id.as_str())).await?;

        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("DELETE FROM users WHERE {}", where_clause);

        let query_builder = sqlx::query(&query);
        let query_builder = ids
            .iter()
            .fold(query_builder, |query_builder, id| query_builder.bind(id));
        query_builder
            .execute(&*self.pool)
            .await
            .map_err(|e| Error::DeleteError(e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use argon2::{
        password_hash::{PasswordHashString, SaltString},
        Argon2, PasswordHasher,
    };
    use rand::rngs::OsRng;
    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use crate::store::{
        common::new_id, user::Query, CompoundOperator, CompoundQuery, Error, LogicalQuery,
    };

    use super::{PasswordType, SqliteStore, Store, User, UserRow};

    use test_case::test_case;

    struct Init {
        db: SqlitePool,
    }

    async fn init() -> Init {
        let db_url = "sqlite://:memory:";
        Sqlite::create_database(db_url);

        let db = SqlitePool::connect_with(
            SqliteConnectOptions::from_str(db_url)
                .unwrap()
                .log_statements(log::LevelFilter::Trace),
        )
        .await
        .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();

        Init { db }
    }

    fn new_password(password: &str) -> PasswordHashString {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .serialize()
    }

    async fn test_data(init: &Init) -> Vec<User> {
        let user1_id = new_id();
        let user1_email = "a@gmail.com";
        let user1_password = new_password("abcde");
        let user1_display_name = "a";
        let user2_id = new_id();
        let user2_email = "b@gmail.com";
        let user2_password = new_password("fghij");
        let user2_display_name = "b";

        let query = "INSERT INTO users (id, email, password, display_name) VALUES (?, ?, ?, ?), (?, ?, ?, ?)";
        sqlx::query(query)
            .bind(&user1_id)
            .bind(user1_email)
            .bind(Some(user1_password.as_str()))
            .bind(user1_display_name)
            .bind(&user2_id)
            .bind(user2_email)
            .bind(Some(user2_password.as_str()))
            .bind(user2_display_name)
            .execute(&init.db)
            .await
            .unwrap();

        let users = vec![
            User {
                id: user1_id,
                email: user1_email.to_string(),
                password: PasswordType::Set(user1_password),
                display_name: user1_display_name.to_string(),
            },
            User {
                id: user2_id,
                email: user2_email.to_string(),
                password: PasswordType::Set(user2_password),
                display_name: user2_display_name.to_string(),
            },
        ];

        users
    }

    #[tokio::test]
    async fn insert() {
        let init = init().await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let users = vec![
            User {
                id: "".to_string(),
                email: "a@gmail.com".to_string(),
                password: PasswordType::Set(new_password("abcde")),
                display_name: "a".to_string(),
            },
            User {
                id: "".to_string(),
                email: "b@gmail.com".to_string(),
                password: PasswordType::Set(new_password("fghij")),
                display_name: "b".to_string(),
            },
        ];

        let returned_users = store.upsert(users.clone()).await.unwrap();

        let mut users = users
            .into_iter()
            .zip(returned_users.iter())
            .map(|(mut org, store_org)| {
                org.id = store_org.id.clone();
                org
            })
            .collect::<Vec<_>>();

        assert_eq!(users, returned_users);

        let store_user_rows: Vec<UserRow> =
            sqlx::query_as("SELECT id, email, password, display_name FROM users")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_users: Vec<User> = store_user_rows
            .into_iter()
            .map(|row| row.try_into().unwrap())
            .collect();

        users.sort_by(|a, b| a.id.cmp(&b.id));
        store_users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(users, store_users);
    }

    #[tokio::test]
    async fn update() {
        let init = init().await;
        let mut users = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        users[0].display_name = "new name".to_string();
        let password_backup = std::mem::take(&mut users[0].password);
        users[1].email = "c@gmail.com".to_string();
        users[1].password = PasswordType::Set(new_password("klmno"));

        let returned_users = store.upsert(users.clone()).await.unwrap();

        assert_eq!(users, returned_users);

        let store_user_rows: Vec<UserRow> =
            sqlx::query_as("SELECT id, email, password, display_name FROM users")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_users: Vec<User> = store_user_rows
            .into_iter()
            .map(|row| row.try_into().unwrap())
            .collect::<Vec<_>>();

        users[0].password = password_backup;
        users.sort_by(|a, b| a.id.cmp(&b.id));
        store_users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(users, store_users);
    }

    #[tokio::test]
    async fn update_does_not_exist() {
        let init = init().await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let id = new_id();

        let result = store
            .upsert(vec![User {
                id: id.clone(),
                email: "whatever".to_string(),
                password: PasswordType::Set(new_password("whatever")),
                display_name: "whatever".to_string(),
            }])
            .await;

        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("unexpected error"),
        }
    }

    enum QueryTest {
        All,
        Id,
        Email,
        PasswordIsSet,
        CompoundQuery,
        NoResults,
    }
    #[test_case(QueryTest::All ; "all")]
    #[test_case(QueryTest::Id ; "id")]
    #[test_case(QueryTest::Email ; "email")]
    #[test_case(QueryTest::PasswordIsSet ; "password is set")]
    #[test_case(QueryTest::CompoundQuery ; "compound query")]
    #[test_case(QueryTest::NoResults ; "no results")]
    #[tokio::test]
    async fn query(test_name: QueryTest) {
        let init = init().await;
        let mut users = test_data(&init).await;

        struct TestCase {
            query: Option<Query>,
            expected: Vec<User>,
        }
        let tc = match test_name {
            QueryTest::All => TestCase {
                query: None,
                expected: users,
            },
            QueryTest::Id => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(users[0].id.clone()))),
                expected: vec![users.remove(0)],
            },
            QueryTest::Email => TestCase {
                query: Some(Query::Email(LogicalQuery::Equals(users[0].email.clone()))),
                expected: vec![users.remove(0)],
            },
            QueryTest::PasswordIsSet => TestCase {
                query: Some(Query::PasswordIsSet(true)),
                expected: users,
            },
            QueryTest::CompoundQuery => TestCase {
                query: Some(Query::CompoundQuery(CompoundQuery {
                    operator: CompoundOperator::Or,
                    queries: users
                        .iter()
                        .map(|user| Query::Email(LogicalQuery::Equals(user.email.clone())))
                        .collect(),
                })),
                expected: users,
            },
            QueryTest::NoResults => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(new_id()))),
                expected: vec![],
            },
        };

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let mut returned_users = store.query(tc.query.as_ref()).await.unwrap();
        returned_users.sort_by(|a, b| a.id.cmp(&b.id));

        let mut expected = tc.expected;
        expected.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(expected, returned_users);
    }

    #[tokio::test]
    async fn delete() {
        let init = init().await;
        let users = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        store.delete(&vec![users[0].id.clone()]).await.unwrap();

        let store_user_rows: Vec<UserRow> =
            sqlx::query_as("SELECT id, email, password, display_name FROM users")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_users: Vec<User> = store_user_rows
            .into_iter()
            .map(|row| row.try_into().unwrap())
            .collect::<Vec<_>>();

        store_users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(users[1], store_users[0]);
    }
}
