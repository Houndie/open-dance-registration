use std::{collections::HashSet, sync::Arc};

use sqlx::SqlitePool;

use common::proto::Event;

use super::{
    common::{ids_in_table, new_id},
    Error,
};

#[derive(sqlx::FromRow)]
struct EventRow {
    id: String,
    name: String,
    organization: String,
}

impl EventRow {
    fn to_event(self) -> Result<Event, Error> {
        Ok(Event {
            id: self.id,
            name: self.name,
            organization_id: self.organization,
        })
    }
}

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn upsert(&self, events: Vec<Event>) -> Result<Vec<Event>, Error>;
    async fn list(&self, event_ids: &Vec<String>) -> Result<Vec<Event>, Error>;
    async fn delete(&self, event_ids: &Vec<String>) -> Result<(), Error>;
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
    async fn upsert(&self, events: Vec<Event>) -> Result<Vec<Event>, Error> {
        let (insert_events, mut update_events): (Vec<_>, Vec<_>) =
            events.into_iter().partition(|e| e.id == "");

        if !update_events.is_empty() {
            // Make sure events exist
            ids_in_table(
                &*self.pool,
                "events",
                update_events.iter().map(|e| e.id.as_str()),
            )
            .await?;
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Error::TransactionStartError(e))?;

        let mut output_events = Vec::new();
        if !insert_events.is_empty() {
            let mut events_with_ids = insert_events
                .into_iter()
                .map(|mut e| {
                    e.id = new_id();
                    e
                })
                .collect::<Vec<_>>();

            let values_clause: String = itertools::Itertools::intersperse(
                events_with_ids.iter().map(|_| "(?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!(
                "INSERT INTO events(id, organization, name) VALUES {}",
                values_clause
            );
            let query_builder = sqlx::query(&query);
            let query_builder =
                events_with_ids
                    .iter()
                    .fold(query_builder, |query_builder, event| {
                        query_builder
                            .bind(&event.id)
                            .bind(&event.organization_id)
                            .bind(&event.name)
                    });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::InsertionError(e))?;
            output_events.append(&mut events_with_ids);
        }

        if !update_events.is_empty() {
            let values_clause: String =
                itertools::Itertools::intersperse(update_events.iter().map(|_| "(?, ?, ?)"), ", ")
                    .collect();

            let query = format!(
                "WITH mydata(id, organization, name) AS (VALUES {}) 
                UPDATE events 
                SET name = mydata.name,
                organization = mydata.organization
                FROM mydata WHERE events.id = mydata.id",
                values_clause
            );
            let query_builder = sqlx::query(&query);
            let query_builder = update_events
                .iter()
                .fold(query_builder, |query_builder, event| {
                    query_builder
                        .bind(&event.id)
                        .bind(&event.organization_id)
                        .bind(&event.name)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;

            output_events.append(&mut update_events);
        }

        tx.commit().await.map_err(|e| Error::TransactionFailed(e))?;

        Ok(output_events)
    }

    async fn list(&self, event_ids: &Vec<String>) -> Result<Vec<Event>, Error> {
        let event_rows: Vec<EventRow> = if event_ids.is_empty() {
            sqlx::query_as("SELECT id, organization, name FROM events")
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?
        } else {
            let where_clause: String =
                itertools::Itertools::intersperse(event_ids.iter().map(|_| "id = ?"), " OR ")
                    .collect();
            let query = format!(
                "SELECT id, organization, name FROM events WHERE {}",
                where_clause
            );

            let query_builder = sqlx::query_as(&query);
            let query_builder = event_ids
                .iter()
                .fold(query_builder, |query_builder, id| query_builder.bind(id));

            let rows: Vec<EventRow> = query_builder
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            if rows.len() < event_ids.len() {
                let found_ids = rows.into_iter().map(|row| row.id).collect::<HashSet<_>>();

                let first_missing = event_ids
                    .iter()
                    .find(|id| !found_ids.contains(*id))
                    .unwrap();
                return Err(Error::IdDoesNotExist(first_missing.clone()));
            };

            rows
        };

        let output_events = event_rows
            .into_iter()
            .map(|row| row.to_event())
            .collect::<Result<_, _>>()?;

        Ok(output_events)
    }

    async fn delete(&self, event_ids: &Vec<String>) -> Result<(), Error> {
        if event_ids.is_empty() {
            return Ok(());
        }

        ids_in_table(
            &*self.pool,
            "events",
            event_ids.iter().map(|id| id.as_str()),
        )
        .await?;

        let where_clause: String =
            itertools::Itertools::intersperse(event_ids.iter().map(|_| "id = ?"), " OR ").collect();
        let query = format!("DELETE FROM events WHERE {}", where_clause);

        let query_builder = sqlx::query(&query);
        let query_builder = event_ids
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
    use std::sync::Arc;

    use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};

    use crate::{proto::Event, store::common::new_id};

    use super::{Error, EventRow, SqliteStore, Store};

    struct Init {
        org: String,
        db: SqlitePool,
    }

    async fn init_db() -> Init {
        let db_url = "sqlite://:memory:";
        Sqlite::create_database(db_url);

        let db = SqlitePool::connect(db_url).await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();

        let org = new_id();
        let org_name = "Organization 1";
        sqlx::query("INSERT INTO organizations(id, name) VALUES (?, ?);")
            .bind(&org)
            .bind(&org_name)
            .execute(&db)
            .await
            .unwrap();

        Init { org, db }
    }

    #[tokio::test]
    async fn insert() {
        let init = init_db().await;
        let db = Arc::new(init.db);

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let event = Event {
            name: "Event 1".to_owned(),
            organization_id: init.org,
            id: "".to_owned(),
        };

        let returned_events = store.upsert(vec![event.clone()]).await.unwrap();

        assert_eq!(returned_events.len(), 1);
        assert_eq!(event.name, returned_events[0].name);

        let mut store_row: Vec<EventRow> =
            sqlx::query_as("SELECT id, organization, name FROM events")
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
        let init = init_db().await;
        let db = Arc::new(init.db);
        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, organization, name) VALUES (?, ?, ?), (?, ?, ?);")
            .bind(&id_1)
            .bind(&init.org)
            .bind(&name_1)
            .bind(&id_2)
            .bind(&init.org)
            .bind(&name_2)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let event = Event {
            name: "Event 3".to_owned(),
            organization_id: init.org,
            id: id_1,
        };

        let returned_events = store.upsert(vec![event.clone()]).await.unwrap();

        assert_eq!(returned_events.len(), 1);
        assert_eq!(event.name, returned_events[0].name);
        assert_eq!(event.id, returned_events[0].id);

        let changed_store_row: Vec<EventRow> =
            sqlx::query_as("SELECT id, organization, name FROM events WHERE id = ?")
                .bind(&event.id)
                .fetch_all(&*db)
                .await
                .unwrap();

        assert_eq!(changed_store_row.len(), 1);
        assert_eq!(changed_store_row[0].name, event.name);
        assert_eq!(changed_store_row[0].id, event.id);

        let unchanged_store_row: Vec<EventRow> =
            sqlx::query_as("SELECT id, organization, name FROM events WHERE id = ?")
                .bind(&id_2)
                .fetch_all(&*db)
                .await
                .unwrap();

        assert_eq!(unchanged_store_row.len(), 1);
        assert_eq!(unchanged_store_row[0].name, name_2);
        assert_eq!(unchanged_store_row[0].id, id_2);
    }

    #[tokio::test]
    async fn update_does_not_exist() {
        let init = init_db().await;
        let db = Arc::new(init.db);

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let event = Event {
            name: "Event 1".to_owned(),
            organization_id: init.org,
            id: new_id(),
        };

        let result = store.upsert(vec![event.clone()]).await;
        match result {
            Ok(_) => panic!("no error returned"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, event.id),
            _ => panic!("incorrect error type: {:?}", result),
        };
    }

    #[tokio::test]
    async fn list_all() {
        let init = init_db().await;
        let db = Arc::new(init.db);
        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, organization, name) VALUES (?, ?, ?), (?, ?, ?);")
            .bind(&id_1)
            .bind(&init.org)
            .bind(&name_1)
            .bind(&id_2)
            .bind(&init.org)
            .bind(&name_2)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let returned_events = store.list(&vec![]).await.unwrap();

        assert_eq!(returned_events.len(), 2);

        match returned_events.iter().find(|e| e.id == id_1) {
            Some(event) => assert_eq!(event.name, name_1),
            None => panic!("id 1 not found in result"),
        };

        match returned_events.iter().find(|e| e.id == id_2) {
            Some(event) => assert_eq!(event.name, name_2),
            None => panic!("id 2 not found in result"),
        };
    }

    #[tokio::test]
    async fn list_some() {
        let init = init_db().await;
        let db = Arc::new(init.db);
        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        let id_3 = new_id();
        let name_3 = "Event 2";
        sqlx::query(
            "INSERT INTO events(id, organization, name) VALUES (?, ?, ?), (?, ?, ?), (?, ?, ?);",
        )
        .bind(&id_1)
        .bind(&init.org)
        .bind(&name_1)
        .bind(&id_2)
        .bind(&init.org)
        .bind(&name_2)
        .bind(&id_3)
        .bind(&init.org)
        .bind(&name_3)
        .execute(&*db)
        .await
        .unwrap();

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let returned_events = store.list(&vec![id_1.clone(), id_2.clone()]).await.unwrap();

        assert_eq!(returned_events.len(), 2);

        match returned_events.iter().find(|e| e.id == id_1) {
            Some(event) => assert_eq!(event.name, name_1),
            None => panic!("id 1 not found in result"),
        };

        match returned_events.iter().find(|e| e.id == id_2) {
            Some(event) => assert_eq!(event.name, name_2),
            None => panic!("id 2 not found in result"),
        };
    }

    #[tokio::test]
    async fn list_some_doesnt_exist() {
        let init = init_db().await;
        let db = Arc::new(init.db);

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let id = new_id();
        let result = store.list(&vec![id.clone()]).await;
        match result {
            Ok(_) => panic!("no error returned"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type: {:?}", result),
        }
    }

    #[tokio::test]
    async fn delete_one() {
        let init = init_db().await;
        let db = Arc::new(init.db);
        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, organization, name) VALUES (?, ?, ?), (?, ?, ?);")
            .bind(&id_1)
            .bind(&init.org)
            .bind(&name_1)
            .bind(&id_2)
            .bind(&init.org)
            .bind(&name_2)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        store.delete(&vec![id_1]).await.unwrap();

        let mut store_row: Vec<EventRow> =
            sqlx::query_as("SELECT id, organization, name FROM events")
                .fetch_all(&*db)
                .await
                .unwrap();

        assert_eq!(store_row.len(), 1);

        let store_event = store_row.pop().unwrap().to_event().unwrap();
        assert_eq!(store_event.name, name_2);
        assert_eq!(store_event.id, id_2);
    }

    #[tokio::test]
    async fn delete_does_not_exist() {
        let db = Arc::new(init_db().await.db);
        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        let id = new_id();
        let result = store.delete(&vec![id.clone()]).await;
        match result {
            Ok(_) => panic!("no error returned"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type: {:?}", result),
        }
    }
}
