use std::{collections::HashSet, sync::Arc};

use sqlx::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

use crate::proto::Event;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("id {0} does not exist")]
    IdDoesNotExist(String),

    #[error("some id does not exist")]
    SomeIdDoesNotExist,

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
    async fn list_events(&self, event_ids: &Vec<String>) -> Result<Vec<Event>, StoreError>;
    async fn delete_events(&self, event_ids: &Vec<String>) -> Result<(), StoreError>;
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
        let (insert_events, update_events): (Vec<_>, Vec<_>) =
            events.into_iter().partition(|e| e.id == "");

        let update_events_with_ids = update_events
            .into_iter()
            .map(|e| {
                let id =
                    Uuid::parse_str(&e.id).map_err(|_| StoreError::IdDoesNotExist(e.id.clone()))?;
                Ok((id, e))
            })
            .collect::<Result<Vec<_>, _>>()?;

        if !update_events_with_ids.is_empty() {
            // Make sure events exist
            ids_in_database(&*self.pool, update_events_with_ids.iter().map(|(id, _)| id)).await?;
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::TransactionStartError(e))?;

        let mut output_events = Vec::new();
        for mut event in insert_events.into_iter() {
            let id = Uuid::now_v7();
            sqlx::query("INSERT INTO events(id, name) VALUES (?, ?);")
                .bind(Into::<Vec<_>>::into(id.as_bytes()))
                .bind(&event.name)
                .execute(&mut *tx)
                .await
                .map_err(|e| StoreError::InsertionError(e))?;

            event.id = uuid_to_string(id);
            output_events.push(event);
        }

        for (id, mut event) in update_events_with_ids.into_iter() {
            sqlx::query("UPDATE events SET name = ? WHERE id = ?")
                .bind(&event.name)
                .bind(Into::<Vec<_>>::into(id.as_bytes()))
                .execute(&mut *tx)
                .await
                .map_err(|e| StoreError::UpdateError(e))?;

            event.id = uuid_to_string(id);
            output_events.push(event);
        }

        tx.commit()
            .await
            .map_err(|e| StoreError::TransactionFailed(e))?;

        Ok(output_events)
    }

    async fn list_events(&self, event_ids: &Vec<String>) -> Result<Vec<Event>, StoreError> {
        let event_rows: Vec<EventRow> = if event_ids.is_empty() {
            sqlx::query_as("SELECT id, name FROM events")
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| StoreError::FetchError(e))?
        } else {
            let valid_ids = event_ids
                .iter()
                .map(|id| Uuid::parse_str(id).map_err(|_| StoreError::IdDoesNotExist(id.clone())))
                .collect::<Result<HashSet<Uuid>, _>>()?;

            let where_clause: String =
                itertools::Itertools::intersperse(valid_ids.iter().map(|_| "id = ?"), " OR ")
                    .collect();
            let query = format!("SELECT id, name FROM events WHERE {}", where_clause);

            let mut query_builder = sqlx::query_as(&query);

            for id in valid_ids.iter() {
                query_builder = query_builder.bind(Into::<Vec<_>>::into(id.as_bytes()))
            }

            let rows: Vec<EventRow> = query_builder
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| StoreError::FetchError(e))?;

            if rows.len() < valid_ids.len() {
                let found_ids = rows
                    .iter()
                    .map(|row| {
                        Ok(Uuid::from_slice(row.id.as_slice())
                            .map_err(|_| StoreError::ColumnParseError("id".to_owned()))?)
                    })
                    .collect::<Result<HashSet<_>, _>>()?;

                let first_missing = valid_ids.iter().find(|id| !found_ids.contains(id)).unwrap();
                return Err(StoreError::IdDoesNotExist(uuid_to_string(*first_missing)));
            };

            rows
        };

        let output_events = event_rows
            .into_iter()
            .map(|row| row.to_event())
            .collect::<Result<_, _>>()?;

        Ok(output_events)
    }

    async fn delete_events(&self, event_ids: &Vec<String>) -> Result<(), StoreError> {
        let valid_ids = event_ids
            .iter()
            .map(|id| Uuid::parse_str(id).map_err(|_| StoreError::IdDoesNotExist(id.clone())))
            .collect::<Result<HashSet<Uuid>, _>>()?;

        ids_in_database(&*self.pool, valid_ids.iter()).await?;

        let select_where_clause: String =
            itertools::Itertools::intersperse(valid_ids.iter().map(|_| "(?)"), ",").collect();

        let query = format!("WITH valid_ids AS (SELECT column1 FROM ( VALUES {} )) SELECT column1 FROM valid_ids LEFT JOIN events ON events.id = valid_ids.column1 WHERE events.id IS NULL", select_where_clause);

        let mut select_query_builder = sqlx::query_as(&query);

        for id in valid_ids.iter() {
            select_query_builder = select_query_builder.bind(Into::<Vec<_>>::into(id.as_bytes()))
        }

        let missing_ids: Vec<(Vec<u8>,)> = select_query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| StoreError::DeleteError(e))?;

        if !missing_ids.is_empty() {
            let id = uuid_to_string(
                Uuid::from_slice(missing_ids[0].0.as_slice())
                    .map_err(|_| StoreError::ColumnParseError("id".to_owned()))?,
            );
            return Err(StoreError::IdDoesNotExist(id));
        };

        let where_clause: String =
            itertools::Itertools::intersperse(valid_ids.iter().map(|_| "id = ?"), " OR ").collect();
        let query = format!("DELETE FROM events WHERE {}", where_clause);

        let mut query_builder = sqlx::query(&query);

        for id in valid_ids.iter() {
            query_builder = query_builder.bind(Into::<Vec<_>>::into(id.as_bytes()))
        }

        query_builder
            .execute(&*self.pool)
            .await
            .map_err(|e| StoreError::DeleteError(e))?;

        Ok(())
    }
}

async fn ids_in_database<'a, Iter>(pool: &SqlitePool, ids: Iter) -> Result<(), StoreError>
where
    Iter: IntoIterator<Item = &'a Uuid> + Clone,
{
    let select_where_clause: String =
        itertools::Itertools::intersperse(ids.clone().into_iter().map(|_| "(?)"), ",").collect();

    let query = format!("WITH valid_ids AS (SELECT column1 FROM ( VALUES {} )) SELECT column1 FROM valid_ids LEFT JOIN events ON events.id = valid_ids.column1 WHERE events.id IS NULL", select_where_clause);

    let mut select_query_builder = sqlx::query_as(&query);

    for id in ids.into_iter() {
        select_query_builder = select_query_builder.bind(Into::<Vec<_>>::into(id.as_bytes()))
    }

    let missing_ids: Vec<(Vec<u8>,)> = select_query_builder
        .fetch_all(pool)
        .await
        .map_err(|e| StoreError::CheckExistsError(e))?;

    if !missing_ids.is_empty() {
        let id = uuid_to_string(
            Uuid::from_slice(missing_ids[0].0.as_slice())
                .map_err(|_| StoreError::ColumnParseError("id".to_owned()))?,
        );
        return Err(StoreError::IdDoesNotExist(id));
    };

    Ok(())
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
            Err(StoreError::IdDoesNotExist(err_id)) => assert_eq!(err_id, uuid_to_string(id)),
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
            Err(StoreError::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type"),
        };
    }

    #[tokio::test]
    async fn list_all() {
        let db = Arc::new(init_db().await);
        let id_1 = Uuid::now_v7();
        let name_1 = "Event 1";
        let id_2 = Uuid::now_v7();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, name) VALUES (?, ?), (?, ?);")
            .bind(Into::<Vec<_>>::into(id_1.as_bytes()))
            .bind(&name_1)
            .bind(Into::<Vec<_>>::into(id_2.as_bytes()))
            .bind(&name_2)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let returned_events = store.list_events(&vec![]).await.unwrap();

        assert_eq!(returned_events.len(), 2);

        match returned_events
            .iter()
            .find(|e| e.id == uuid_to_string(id_1))
        {
            Some(event) => assert_eq!(event.name, name_1),
            None => panic!("id 1 not found in result"),
        };

        match returned_events
            .iter()
            .find(|e| e.id == uuid_to_string(id_2))
        {
            Some(event) => assert_eq!(event.name, name_2),
            None => panic!("id 2 not found in result"),
        };
    }

    #[tokio::test]
    async fn list_some() {
        let db = Arc::new(init_db().await);
        let id_1 = Uuid::now_v7();
        let name_1 = "Event 1";
        let id_2 = Uuid::now_v7();
        let name_2 = "Event 2";
        let id_3 = Uuid::now_v7();
        let name_3 = "Event 2";
        sqlx::query("INSERT INTO events(id, name) VALUES (?, ?), (?, ?), (?, ?);")
            .bind(Into::<Vec<_>>::into(id_1.as_bytes()))
            .bind(&name_1)
            .bind(Into::<Vec<_>>::into(id_2.as_bytes()))
            .bind(&name_2)
            .bind(Into::<Vec<_>>::into(id_3.as_bytes()))
            .bind(&name_3)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let returned_events = store
            .list_events(&vec![uuid_to_string(id_1), uuid_to_string(id_2)])
            .await
            .unwrap();

        assert_eq!(returned_events.len(), 2);

        match returned_events
            .iter()
            .find(|e| e.id == uuid_to_string(id_1))
        {
            Some(event) => assert_eq!(event.name, name_1),
            None => panic!("id 1 not found in result"),
        };

        match returned_events
            .iter()
            .find(|e| e.id == uuid_to_string(id_2))
        {
            Some(event) => assert_eq!(event.name, name_2),
            None => panic!("id 2 not found in result"),
        };
    }

    #[tokio::test]
    async fn list_some_bad_id() {
        let db = Arc::new(init_db().await);

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let id = "notauuid".to_owned();
        match store.list_events(&vec![id.clone()]).await {
            Ok(_) => panic!("no error returned"),
            Err(StoreError::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type"),
        }
    }

    #[tokio::test]
    async fn list_some_doesnt_exist() {
        let db = Arc::new(init_db().await);

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let id = uuid_to_string(Uuid::now_v7());
        match store.list_events(&vec![id.clone()]).await {
            Ok(_) => panic!("no error returned"),
            Err(StoreError::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type"),
        }
    }

    #[tokio::test]
    async fn delete_one() {
        let db = Arc::new(init_db().await);
        let id_1 = Uuid::now_v7();
        let name_1 = "Event 1";
        let id_2 = Uuid::now_v7();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, name) VALUES (?, ?), (?, ?);")
            .bind(Into::<Vec<_>>::into(id_1.as_bytes()))
            .bind(&name_1)
            .bind(Into::<Vec<_>>::into(id_2.as_bytes()))
            .bind(&name_2)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        store
            .delete_events(&vec![uuid_to_string(id_1)])
            .await
            .unwrap();

        let mut store_row: Vec<EventRow> = sqlx::query_as("SELECT id, name FROM events")
            .fetch_all(&*db)
            .await
            .unwrap();

        assert_eq!(store_row.len(), 1);

        let store_event = store_row.pop().unwrap().to_event().unwrap();
        assert_eq!(store_event.name, name_2);
        assert_eq!(store_event.id, uuid_to_string(id_2));
    }

    #[tokio::test]
    async fn delete_bad_id() {
        let db = Arc::new(init_db().await);
        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let id = "notauuid".to_owned();
        match store.delete_events(&vec![id.clone()]).await {
            Ok(_) => panic!("no error returned"),
            Err(StoreError::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type"),
        }
    }

    #[tokio::test]
    async fn delete_does_not_exist() {
        let db = Arc::new(init_db().await);
        let store = {
            let db = db.clone();
            SqliteEventStore::new(db)
        };

        let id = uuid_to_string(Uuid::now_v7());
        match store.delete_events(&vec![id.clone()]).await {
            Ok(_) => panic!("no error returned"),
            Err(StoreError::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type"),
        }
    }
}
