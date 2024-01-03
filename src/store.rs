use std::sync::Arc;

use sqlx::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

use crate::proto::Event;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("id {0} does not exist")]
    IDDoesNotExist(String),

    #[error("error inserting new event into data store: {0}")]
    InsertionError(#[source] sqlx::Error),

    #[error("error fetching event from database: {0}")]
    FetchError(#[source] sqlx::Error),

    #[error("error updating event: {0}")]
    UpdateError(#[source] sqlx::Error),

    #[error("transaction failed to commit: {0}")]
    TransactionFailed(#[source] sqlx::Error),

    #[error("transaction failed to start: {0}")]
    TransactionStartError(#[source] sqlx::Error),

    #[error("unable to parse column {0}")]
    ColumnParseError(String),
}

fn uuid_to_string(id: Uuid) -> String {
    id.hyphenated()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_owned()
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Vec<u8>,
    name: String,
}

impl EventRow {
    fn to_event(self) -> Result<Event, StoreError> {
        Ok(Event {
            id: uuid_to_string(
                Uuid::from_slice(self.id.as_slice())
                    .map_err(|_| StoreError::ColumnParseError("id".to_owned()))?,
            ),
            name: self.name,
            registration_schema: None,
        })
    }
}

#[tonic::async_trait]
pub trait EventStore: Send + Sync + 'static {
    async fn upsert_events(&self, events: Vec<Event>) -> Result<Vec<Event>, StoreError>;
}

#[derive(Debug)]
pub struct SqliteEventStore {
    pool: Arc<SqlitePool>,
}

impl SqliteEventStore {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        SqliteEventStore { pool }
    }
}

#[tonic::async_trait]
impl EventStore for SqliteEventStore {
    async fn upsert_events(&self, events: Vec<Event>) -> Result<Vec<Event>, StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::TransactionStartError(e))?;

        let mut output_events = Vec::new();
        for mut event in events.into_iter() {
            let id = if event.id == "" {
                let id = Uuid::now_v7();
                sqlx::query("INSERT INTO events(id, name) VALUES (?, ?);")
                    .bind(Into::<Vec<_>>::into(id.as_bytes()))
                    .bind(&event.name)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| StoreError::InsertionError(e))?;

                uuid_to_string(id)
            } else {
                let id = Uuid::parse_str(&event.id)
                    .map_err(|_| StoreError::IDDoesNotExist(event.id.clone()))?;

                let event_rows: Vec<EventRow> =
                    sqlx::query_as("SELECT id, name FROM events WHERE id = ?")
                        .bind(Into::<Vec<_>>::into(id.as_bytes()))
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(|e| StoreError::FetchError(e))?;

                if event_rows.is_empty() {
                    return Err(StoreError::IDDoesNotExist(event.id.clone()));
                }

                sqlx::query("UPDATE events SET name = ? WHERE id = ?")
                    .bind(&event.name)
                    .bind(Into::<Vec<_>>::into(id.as_bytes()))
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| StoreError::UpdateError(e))?;

                event.id
            };

            event.id = id;
            output_events.push(event);
        }

        tx.commit()
            .await
            .map_err(|e| StoreError::TransactionFailed(e))?;

        Ok(output_events)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
    use uuid::Uuid;

    use crate::proto::Event;

    use super::{uuid_to_string, EventRow, EventStore, SqliteEventStore, StoreError};

    async fn init_db() -> SqlitePool {
        let db_url = "sqlite://:memory:";
        Sqlite::create_database(db_url);

        let db = SqlitePool::connect(db_url).await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn insert() {
        let db = Arc::new(init_db().await);

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let event = Event {
            name: "Event 1".to_owned(),
            id: "".to_owned(),
            registration_schema: None,
        };

        let returned_events = store.upsert_events(vec![event.clone()]).await.unwrap();

        assert_eq!(returned_events.len(), 1);
        assert_eq!(event.name, returned_events[0].name);

        let mut store_row: Vec<EventRow> = sqlx::query_as("SELECT id, name FROM events")
            .fetch_all(&*db)
            .await
            .unwrap();

        assert_eq!(store_row.len(), 1);

        let store_event = store_row.pop().unwrap().to_event().unwrap();
        assert_eq!(store_event.name, event.name);
        assert_eq!(store_event.id, returned_events[0].id);
    }

    #[tokio::test]
    async fn update() {
        let db = Arc::new(init_db().await);
        let id = Uuid::now_v7();
        let name = "Event 1";
        sqlx::query("INSERT INTO events(id, name) VALUES (?, ?);")
            .bind(Into::<Vec<_>>::into(id.as_bytes()))
            .bind(&name)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let event = Event {
            name: "Event 2".to_owned(),
            id: uuid_to_string(id),
            registration_schema: None,
        };

        let returned_events = store.upsert_events(vec![event.clone()]).await.unwrap();

        assert_eq!(returned_events.len(), 1);
        assert_eq!(event.name, returned_events[0].name);

        let mut store_row: Vec<EventRow> = sqlx::query_as("SELECT id, name FROM events")
            .fetch_all(&*db)
            .await
            .unwrap();

        assert_eq!(store_row.len(), 1);

        let store_event = store_row.pop().unwrap().to_event().unwrap();
        assert_eq!(store_event.name, event.name);
        assert_eq!(store_event.id, returned_events[0].id);
    }

    #[tokio::test]
    async fn update_does_not_exist() {
        let db = Arc::new(init_db().await);

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let id = Uuid::now_v7();
        let event = Event {
            name: "Event 1".to_owned(),
            id: uuid_to_string(id),
            registration_schema: None,
        };

        match store.upsert_events(vec![event.clone()]).await {
            Ok(_) => panic!("no error returned"),
            Err(StoreError::IDDoesNotExist(err_id)) => assert_eq!(err_id, uuid_to_string(id)),
            _ => panic!("incorrect error type"),
        };
    }

    #[tokio::test]
    async fn update_bad_id() {
        let db = Arc::new(init_db().await);

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let id = "notauuid".to_owned();
        let event = Event {
            name: "Event 1".to_owned(),
            id: id.clone(),
            registration_schema: None,
        };

        match store.upsert_events(vec![event.clone()]).await {
            Ok(_) => panic!("no error returned"),
            Err(StoreError::IDDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type"),
        };
    }
}
