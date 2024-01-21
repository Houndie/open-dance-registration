use std::sync::Arc;

use bcrypt::BcryptError;
use sqlx::SqlitePool;

use super::{
    common::{ids_in_table, new_id},
    Error,
};

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    email: String,
    password: Option<String>,
    display_name: String,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        let password = match row.password {
            Some(password) => PasswordType::Set(HashedPassword(password)),
            None => PasswordType::Unset,
        };

        User {
            id: row.id,
            email: row.email,
            password,
            display_name: row.display_name,
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct HashedPassword(String);

impl HashedPassword {
    pub fn new(password: String) -> Result<Self, BcryptError> {
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;

        Ok(HashedPassword(hash))
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub enum PasswordType {
    Set(HashedPassword),
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

type QueryBuilder<'q> = sqlx::query::Query<
    'q,
    sqlx::Sqlite,
    <sqlx::Sqlite as sqlx::database::HasArguments<'q>>::Arguments,
>;

fn bind_user<'q>(query_builder: QueryBuilder<'q>, user: &'q User) -> QueryBuilder<'q> {
    let query_builder = query_builder.bind(&user.id).bind(&user.email);

    let query_builder = match &user.password {
        PasswordType::Set(password) => query_builder.bind(Some(&password.0)),
        PasswordType::Unset => query_builder.bind(None as Option<&str>),
        PasswordType::Unchanged => query_builder,
    };

    query_builder.bind(&user.display_name)
}

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn upsert(&self, users: Vec<User>) -> Result<Vec<User>, Error>;
    async fn list(&self, ids: Vec<String>) -> Result<Vec<User>, Error>;
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

    async fn list(&self, ids: Vec<String>) -> Result<Vec<User>, Error> {
        let base_query = "SELECT id, email, password, display_name FROM users";
        if ids.is_empty() {
            let users: Vec<UserRow> = sqlx::query_as(base_query)
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            return Ok(users.into_iter().map(|row| row.into()).collect());
        };

        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("{} WHERE {}", base_query, where_clause);

        let query_builder = sqlx::query_as(&query);
        let query_builder = ids
            .iter()
            .fold(query_builder, |query_builder, id| query_builder.bind(id));

        let rows: Vec<UserRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| Error::FetchError(e))?;

        Ok(rows.into_iter().map(|row| row.into()).collect())
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

    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use crate::store::{common::new_id, Error};

    use super::{HashedPassword, PasswordType, SqliteStore, Store, User, UserRow};

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

    async fn test_data(init: &Init) -> Vec<User> {
        let user1_id = new_id();
        let user1_email = "a@gmail.com";
        let user1_password = "abcde";
        let user1_display_name = "a";
        let user2_id = new_id();
        let user2_email = "b@gmail.com";
        let user2_password = "fghij";
        let user2_display_name = "b";

        let query = "INSERT INTO users (id, email, password, display_name) VALUES (?, ?, ?, ?), (?, ?, ?, ?)";
        sqlx::query(query)
            .bind(&user1_id)
            .bind(user1_email)
            .bind(user1_password)
            .bind(user1_display_name)
            .bind(&user2_id)
            .bind(user2_email)
            .bind(user2_password)
            .bind(user2_display_name)
            .execute(&init.db)
            .await
            .unwrap();

        let users = vec![
            User {
                id: user1_id,
                email: user1_email.to_string(),
                password: PasswordType::Set(HashedPassword(user1_password.to_string())),
                display_name: user1_display_name.to_string(),
            },
            User {
                id: user2_id,
                email: user2_email.to_string(),
                password: PasswordType::Set(HashedPassword(user2_password.to_string())),
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
                password: PasswordType::Set(HashedPassword("abcde".to_string())),
                display_name: "a".to_string(),
            },
            User {
                id: "".to_string(),
                email: "b@gmail.com".to_string(),
                password: PasswordType::Set(HashedPassword("fghij".to_string())),
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

        let mut store_users: Vec<User> =
            store_user_rows.into_iter().map(|row| row.into()).collect();

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
        users[1].password = PasswordType::Set(HashedPassword("klmno".to_string()));

        let returned_users = store.upsert(users.clone()).await.unwrap();

        assert_eq!(users, returned_users);

        let store_user_rows: Vec<UserRow> =
            sqlx::query_as("SELECT id, email, password, display_name FROM users")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_users: Vec<User> = store_user_rows
            .into_iter()
            .map(|row| row.into())
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
                password: PasswordType::Set(HashedPassword("whatever".to_string())),
                display_name: "whatever".to_string(),
            }])
            .await;

        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("unexpected error"),
        }
    }

    #[tokio::test]
    async fn list_all() {
        let init = init().await;
        let mut users = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let mut returned_users = store.list(vec![]).await.unwrap();

        users.sort_by(|a, b| a.id.cmp(&b.id));
        returned_users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(users, returned_users);
    }

    #[tokio::test]
    async fn list_some() {
        let init = init().await;
        let users = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let returned_users = store.list(vec![users[0].id.clone()]).await.unwrap();

        assert_eq!(users[0], returned_users[0]);
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
            .map(|row| row.into())
            .collect::<Vec<_>>();

        store_users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(users[1], store_users[0]);
    }
}
