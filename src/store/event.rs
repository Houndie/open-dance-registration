use crate::{
    proto::Event,
    store::{
        common::{ids_in_table, new_id},
        Bindable as _, Error, Queryable as _,
    },
};
use sqlx::SqlitePool;
use std::{future::Future, sync::Arc};

#[derive(sqlx::FromRow)]
struct EventRow {
    id: String,
    name: String,
    organization: String,
}

impl From<EventRow> for Event {
    fn from(row: EventRow) -> Self {
        Event {
            id: row.id,
            name: row.name,
            organization_id: row.organization,
        }
    }
}

pub struct IdField;

impl super::Field for IdField {
    type Item = String;

    fn field() -> &'static str {
        "id"
    }
}

pub type IdQuery = super::LogicalQuery<IdField>;

pub struct OrganizationField;

impl super::Field for OrganizationField {
    type Item = String;

    fn field() -> &'static str {
        "organization"
    }
}

pub type OrganizationQuery = super::LogicalQuery<OrganizationField>;

pub enum Query {
    Id(IdQuery),
    Organization(OrganizationQuery),
    CompoundQuery(super::CompoundQuery<Query>),
}

impl super::Queryable for Query {
    fn where_clause(&self) -> String {
        match self {
            Query::Id(q) => q.where_clause(),
            Query::Organization(q) => q.where_clause(),
            Query::CompoundQuery(compound_query) => compound_query.where_clause(),
        }
    }
}

impl<'q, DB: sqlx::Database> super::Bindable<'q, DB> for Query
where
    <IdField as super::Field>::Item: sqlx::Type<DB> + sqlx::Encode<'q, DB>,
    <OrganizationField as super::Field>::Item: sqlx::Type<DB> + sqlx::Encode<'q, DB>,
{
    fn bind<O>(
        &'q self,
        query_builder: sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>>,
    ) -> sqlx::query::QueryAs<'q, DB, O, <DB as sqlx::Database>::Arguments<'q>> {
        match self {
            Query::Id(q) => q.bind(query_builder),
            Query::Organization(q) => q.bind(query_builder),
            Query::CompoundQuery(compound_query) => compound_query.bind(query_builder),
        }
    }
}

pub trait Store: Send + Sync + 'static {
    fn upsert(&self, events: Vec<Event>) -> impl Future<Output = Result<Vec<Event>, Error>> + Send;
    fn query(
        &self,
        query: Option<&Query>,
    ) -> impl Future<Output = Result<Vec<Event>, Error>> + Send;
    fn delete(&self, event_ids: &[String]) -> impl Future<Output = Result<(), Error>> + Send;
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

impl Store for SqliteStore {
    async fn upsert(&self, events: Vec<Event>) -> Result<Vec<Event>, Error> {
        let (insert_events, mut update_events): (Vec<_>, Vec<_>) =
            events.into_iter().partition(|e| e.id.is_empty());

        if !update_events.is_empty() {
            // Make sure events exist
            ids_in_table(
                &self.pool,
                "events",
                update_events.iter().map(|e| e.id.as_str()),
            )
            .await?;
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(Error::TransactionStartError)?;

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
                .map_err(Error::InsertionError)?;
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
                .map_err(Error::UpdateError)?;

            output_events.append(&mut update_events);
        }

        tx.commit().await.map_err(Error::TransactionFailed)?;

        Ok(output_events)
    }

    async fn query(&self, query: Option<&Query>) -> Result<Vec<Event>, Error> {
        let base_query = "SELECT id, organization, name FROM events";
        let query_string = match query {
            Some(query) => format!("{} WHERE {}", base_query, query.where_clause()),
            None => base_query.to_owned(),
        };

        let query_builder = sqlx::query_as(&query_string);
        let query_builder = match query {
            Some(query) => query.bind(query_builder),
            None => query_builder,
        };

        let rows: Vec<EventRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(Error::FetchError)?;

        let output_events = rows.into_iter().map(|row| row.into()).collect();

        Ok(output_events)
    }

    async fn delete(&self, event_ids: &[String]) -> Result<(), Error> {
        if event_ids.is_empty() {
            return Ok(());
        }

        ids_in_table(&self.pool, "events", event_ids.iter().map(|id| id.as_str())).await?;

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
            .map_err(Error::DeleteError)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, EventRow, Query, SqliteStore, Store};
    use crate::{
        proto::Event,
        store::{common::new_id, CompoundOperator, CompoundQuery, LogicalQuery},
    };
    use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
    use std::sync::Arc;

    struct Init {
        org: String,
        db: SqlitePool,
    }

    use test_case::test_case;

    async fn init_db() -> Init {
        let db_url = "sqlite://:memory:";
        Sqlite::create_database(db_url).await.unwrap();

        let db = SqlitePool::connect(db_url).await.unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();

        let org = new_id();
        let org_name = "Organization 1";
        sqlx::query("INSERT INTO organizations(id, name) VALUES (?, ?);")
            .bind(&org)
            .bind(org_name)
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

        let store_event: Event = store_row.pop().unwrap().into();
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
            .bind(name_1)
            .bind(&id_2)
            .bind(&init.org)
            .bind(name_2)
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

    enum QueryTest {
        All,
        Id,
        Organization,
        CompoundQuery,
        NoResults,
    }
    #[test_case(QueryTest::All ; "all")]
    #[test_case(QueryTest::Id ; "id")]
    #[test_case(QueryTest::Organization ; "organization")]
    #[test_case(QueryTest::CompoundQuery ; "compound query")]
    #[test_case(QueryTest::NoResults ; "no results")]
    #[tokio::test]
    async fn query(test_name: QueryTest) {
        let init = init_db().await;
        let db = Arc::new(init.db);
        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, organization, name) VALUES (?, ?, ?), (?, ?, ?);")
            .bind(&id_1)
            .bind(&init.org)
            .bind(name_1)
            .bind(&id_2)
            .bind(&init.org)
            .bind(name_2)
            .execute(&*db)
            .await
            .unwrap();

        let mut events = vec![
            Event {
                name: name_1.to_owned(),
                organization_id: init.org.clone(),
                id: id_1.clone(),
            },
            Event {
                name: name_2.to_owned(),
                organization_id: init.org.clone(),
                id: id_2.clone(),
            },
        ];

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        struct TestCase {
            query: Option<Query>,
            expected: Vec<Event>,
        }

        let tc = match test_name {
            QueryTest::All => TestCase {
                query: None,
                expected: events,
            },
            QueryTest::Id => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(id_1))),
                expected: vec![events.remove(0)],
            },
            QueryTest::Organization => TestCase {
                query: Some(Query::Organization(LogicalQuery::Equals(init.org))),
                expected: events,
            },
            QueryTest::CompoundQuery => TestCase {
                query: Some(Query::CompoundQuery(CompoundQuery {
                    operator: CompoundOperator::Or,
                    queries: events
                        .iter()
                        .map(|e| Query::Id(LogicalQuery::Equals(e.id.clone())))
                        .collect(),
                })),
                expected: events,
            },
            QueryTest::NoResults => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(new_id()))),
                expected: vec![],
            },
        };

        let mut returned_events = store.query(tc.query.as_ref()).await.unwrap();
        returned_events.sort_by(|a, b| a.id.cmp(&b.id));
        let mut expected_events = tc.expected;
        expected_events.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(returned_events.len(), expected_events.len());
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
            .bind(name_1)
            .bind(&id_2)
            .bind(&init.org)
            .bind(name_2)
            .execute(&*db)
            .await
            .unwrap();

        let store = {
            let db = db.clone();
            SqliteStore::new(db)
        };

        store.delete(&[id_1]).await.unwrap();

        let mut store_row: Vec<EventRow> =
            sqlx::query_as("SELECT id, organization, name FROM events")
                .fetch_all(&*db)
                .await
                .unwrap();

        assert_eq!(store_row.len(), 1);

        let store_event: Event = store_row.pop().unwrap().into();
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
        let result = store.delete(&[id.clone()]).await;
        match result {
            Ok(_) => panic!("no error returned"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type: {:?}", result),
        }
    }
}
