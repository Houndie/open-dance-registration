use std::sync::Arc;

use chrono::{DateTime, Utc};
use ed25519_dalek::{SecretKey, SigningKey};
use sqlx::SqlitePool;

use super::{common::new_id, Error};

#[derive(Debug, PartialEq)]
pub struct Key {
    pub id: String,
    pub key: SigningKey,
    pub created_at: DateTime<Utc>,
}

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn insert(&self, mut key: Key) -> Result<Key, Error>;
    async fn list(&self, ids: Vec<&str>) -> Result<Vec<Key>, Error>;
    async fn get_newest(&self) -> Result<Key, Error>;
    async fn has(&self) -> Result<bool, Error>;
    async fn delete(&self, ids: Vec<String>) -> Result<(), Error>;
}

pub struct SqliteStore {
    pool: Arc<SqlitePool>,
}

impl SqliteStore {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        SqliteStore { pool }
    }
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: String,
    eddsa_key: Vec<u8>,
    created_at: i64,
}

impl TryFrom<KeyRow> for Key {
    type Error = Error;

    fn try_from(row: KeyRow) -> Result<Self, Self::Error> {
        let key_bytes: SecretKey = row
            .eddsa_key
            .try_into()
            .map_err(|_| Error::ColumnParseError("eddsa_key"))?;

        Ok(Key {
            id: row.id,
            key: SigningKey::from_bytes(&key_bytes),
            created_at: DateTime::<Utc>::from_timestamp(row.created_at, 0)
                .ok_or_else(|| Error::ColumnParseError("created_at"))?,
        })
    }
}

#[tonic::async_trait]
impl Store for SqliteStore {
    async fn insert(&self, mut key: Key) -> Result<Key, Error> {
        key.id = new_id();

        sqlx::query(
            r#"
            INSERT INTO keys (id, eddsa_key, created_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(&key.id)
        .bind(key.key.to_bytes().as_slice())
        .bind(key.created_at.timestamp())
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::InsertionError(e))?;

        Ok(key)
    }

    async fn get_newest(&self) -> Result<Key, Error> {
        let row: KeyRow = sqlx::query_as(
            r#"
            SELECT id, eddsa_key, MAX(created_at) AS created_at
            FROM keys
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| Error::FetchError(e))?;

        row.try_into()
    }

    async fn has(&self) -> Result<bool, Error> {
        let row: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS(SELECT 1 FROM keys)
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| Error::FetchError(e))?;

        Ok(row.0)
    }

    async fn list(&self, ids: Vec<&str>) -> Result<Vec<Key>, Error> {
        let base_query = "SELECT id, eddsa_key, created_at FROM keys";
        if ids.is_empty() {
            let rows: Vec<KeyRow> = sqlx::query_as(base_query)
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            let keys = rows
                .into_iter()
                .map(|r| r.try_into())
                .collect::<Result<Vec<_>, _>>()?;

            return Ok(keys);
        };

        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("{} WHERE {}", base_query, where_clause);

        let query_builder = sqlx::query_as(&query);
        let query_builder = ids.iter().fold(query_builder, |qb, id| qb.bind(id));
        let rows: Vec<KeyRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| Error::FetchError(e))?;

        Ok(rows
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?)
    }

    async fn delete(&self, ids: Vec<String>) -> Result<(), Error> {
        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("DELETE FROM keys WHERE {}", where_clause);
        let query_builder = sqlx::query(&query);
        let query_builder = ids.iter().fold(query_builder, |qb, id| qb.bind(id));
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

    use chrono::{DateTime, Utc};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use super::{Key, SqliteStore, Store};
    use crate::store::common::new_id;

    struct Init {
        db: SqlitePool,
    }

    async fn init_db() -> Init {
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

    #[tokio::test]
    async fn get_newest() {
        let old_key = Key {
            id: new_id(),
            key: SigningKey::generate(&mut OsRng),
            created_at: DateTime::<Utc>::from_timestamp(1000, 0).unwrap(),
        };

        let new_key = Key {
            id: new_id(),
            key: SigningKey::generate(&mut OsRng),
            created_at: DateTime::<Utc>::from_timestamp(3000, 0).unwrap(),
        };

        let init = init_db().await;

        sqlx::query(
            r#"
            INSERT INTO keys (id, eddsa_key, created_at)
            VALUES (?, ?, ?), (?, ?, ?)
            "#,
        )
        .bind(&old_key.id)
        .bind(old_key.key.to_bytes().as_slice())
        .bind(old_key.created_at.timestamp())
        .bind(&new_key.id)
        .bind(new_key.key.to_bytes().as_slice())
        .bind(new_key.created_at.timestamp())
        .execute(&init.db)
        .await
        .unwrap();

        let store = SqliteStore::new(Arc::new(init.db));

        let returned_key = store.get_newest().await.unwrap();

        assert_eq!(returned_key, new_key);
    }
}
