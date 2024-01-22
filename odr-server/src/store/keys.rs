use std::sync::Arc;

use chrono::{DateTime, Utc};
use p256::ecdsa;
use sqlx::SqlitePool;

use super::{common::new_id, Error};

pub struct Key {
    pub id: String,
    pub key: ecdsa::SigningKey,
    pub expires_at: DateTime<Utc>,
}

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn insert(&self, mut key: Key) -> Result<Key, Error>;
    async fn list(&self) -> Result<Vec<Key>, Error>;
    async fn delete(&self, id: String) -> Result<(), Error>;
}

pub struct SqliteStore {
    pool: Arc<SqlitePool>,
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: String,
    ecdsa_key: Vec<u8>,
    expires_at: i64,
}

impl TryFrom<KeyRow> for Key {
    type Error = Error;

    fn try_from(row: KeyRow) -> Result<Self, Self::Error> {
        Ok(Key {
            id: row.id,
            key: ecdsa::SigningKey::from_slice(row.ecdsa_key.as_slice())
                .map_err(|_| Error::ColumnParseError("ecdsa_key"))?,
            expires_at: DateTime::<Utc>::from_timestamp(row.expires_at, 0)
                .ok_or_else(|| Error::ColumnParseError("expires_at"))?,
        })
    }
}

#[tonic::async_trait]
impl Store for SqliteStore {
    async fn insert(&self, mut key: Key) -> Result<Key, Error> {
        key.id = new_id();

        sqlx::query(
            r#"
            INSERT INTO keys (id, ecdsa_key, expires_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(&key.id)
        .bind(key.key.to_bytes().as_slice())
        .bind(key.expires_at.timestamp())
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::InsertionError(e))?;

        Ok(key)
    }

    async fn list(&self) -> Result<Vec<Key>, Error> {
        let rows: Vec<KeyRow> = sqlx::query_as(
            r#"
            SELECT id, ecdsa_key, expires_at
            FROM keys
            "#,
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Error::FetchError(e))?;

        let keys = rows
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(keys)
    }

    async fn delete(&self, id: String) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM keys
            WHERE id = ?
            "#,
        )
        .bind(&id)
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::DeleteError(e))?;

        Ok(())
    }
}
