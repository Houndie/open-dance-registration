use std::{future::Future, sync::Arc};

use super::{
    common::{ids_in_table, new_id},
    Bindable as _, Error, Queryable as _,
};
use common::proto::Organization;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow)]
struct OrganizationRow {
    id: String,
    name: String,
}

impl From<OrganizationRow> for Organization {
    fn from(row: OrganizationRow) -> Self {
        Organization {
            id: row.id,
            name: row.name,
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

pub enum Query {
    Id(IdQuery),
    CompoundQuery(super::CompoundQuery<Query>),
}

impl super::Queryable for Query {
    fn where_clause(&self) -> String {
        match self {
            Query::Id(q) => q.where_clause(),
            Query::CompoundQuery(compound_query) => compound_query.where_clause(),
        }
    }
}

impl<'q, DB: sqlx::Database> super::Bindable<'q, DB> for Query
where
    <IdField as super::Field>::Item: sqlx::Type<DB> + sqlx::Encode<'q, DB>,
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
            Query::CompoundQuery(compound_query) => compound_query.bind(query_builder),
        }
    }
}

pub trait Store: Send + Sync + 'static {
    fn upsert(
        &self,
        organizations: Vec<Organization>,
    ) -> impl Future<Output = Result<Vec<Organization>, Error>> + Send;
    fn query(
        &self,
        query: Option<&Query>,
    ) -> impl Future<Output = Result<Vec<Organization>, Error>> + Send;
    fn delete(&self, ids: &[String]) -> impl Future<Output = Result<(), Error>> + Send;
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
    async fn upsert(&self, organizations: Vec<Organization>) -> Result<Vec<Organization>, Error> {
        let (inserts, updates): (Vec<_>, Vec<_>) = organizations
            .into_iter()
            .enumerate()
            .partition(|(_, org)| org.id.is_empty());

        if !updates.is_empty() {
            // Make sure events exist
            ids_in_table(
                &self.pool,
                "organizations",
                updates.iter().map(|(_, org)| org.id.as_str()),
            )
            .await?;
        }

        let inserts = inserts
            .into_iter()
            .map(|(idx, mut org)| {
                org.id = new_id();
                (idx, org)
            })
            .collect::<Vec<_>>();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(Error::TransactionStartError)?;

        if !inserts.is_empty() {
            let values_clause = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?)").take(inserts.len()),
                " , ",
            )
            .collect::<String>();

            let query = format!(
                "INSERT INTO organizations (id, name) VALUES {}",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = inserts
                .iter()
                .fold(query_builder, |query_builder, (_, org)| {
                    query_builder.bind(&org.id).bind(&org.name)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(Error::InsertionError)?;
        };

        if !updates.is_empty() {
            let values_clause = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?)").take(updates.len()),
                " , ",
            )
            .collect::<String>();

            let query = format!(
                "WITH mydata(id, name) AS (VALUES {}) 
                    UPDATE organizations 
                    SET name = mydata.name 
                    FROM mydata 
                    WHERE organizations.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = updates
                .iter()
                .fold(query_builder, |query_builder, (_, org)| {
                    query_builder.bind(&org.id).bind(&org.name)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(Error::UpdateError)?;
        };

        tx.commit().await.map_err(Error::TransactionFailed)?;

        let mut outputs = Vec::new();
        outputs.resize(inserts.len() + updates.len(), Organization::default());
        inserts.into_iter().chain(updates).for_each(|(idx, org)| {
            outputs[idx] = org;
        });

        Ok(outputs)
    }
    async fn query(&self, query: Option<&Query>) -> Result<Vec<Organization>, Error> {
        let base_query = "SELECT id, name FROM organizations";
        let query_string = match query {
            Some(query) => format!("{} WHERE {}", base_query, query.where_clause()),
            None => base_query.to_string(),
        };

        let query_builder = sqlx::query_as(&query_string);
        let query_builder = match query {
            Some(query) => query.bind(query_builder),
            None => query_builder,
        };

        let rows: Vec<OrganizationRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(Error::FetchError)?;

        Ok(rows.into_iter().map(|row| row.into()).collect())
    }

    async fn delete(&self, ids: &[String]) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }

        ids_in_table(
            &self.pool,
            "organizations",
            ids.iter().map(|id| id.as_str()),
        )
        .await?;

        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("DELETE FROM organizations WHERE {}", where_clause);

        let query_builder = sqlx::query(&query);
        let query_builder = ids
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
    use std::{str::FromStr, sync::Arc};

    use common::proto::Organization;
    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use crate::store::{common::new_id, CompoundOperator, CompoundQuery, Error, LogicalQuery};

    use super::{OrganizationRow, Query, SqliteStore, Store};

    use test_case::test_case;

    struct Init {
        db: SqlitePool,
    }

    async fn init() -> Init {
        let db_url = "sqlite://:memory:";
        Sqlite::create_database(db_url).await.unwrap();

        let db = SqlitePool::connect_with(
            SqliteConnectOptions::from_str(db_url)
                .unwrap()
                .log_statements(log::LevelFilter::Trace),
        )
        .await
        .unwrap();
        sqlx::migrate!("../migrations").run(&db).await.unwrap();

        Init { db }
    }

    async fn test_data(init: &Init) -> Vec<Organization> {
        let org1_id = new_id();
        let org1_name = "org1";
        let org2_id = new_id();
        let org2_name = "org2";

        let query = "INSERT INTO organizations (id, name) VALUES (?, ?), (?, ?)";
        sqlx::query(query)
            .bind(&org1_id)
            .bind(org1_name)
            .bind(&org2_id)
            .bind(org2_name)
            .execute(&init.db)
            .await
            .unwrap();

        let organizations = vec![
            Organization {
                id: org1_id,
                name: org1_name.to_string(),
            },
            Organization {
                id: org2_id,
                name: org2_name.to_string(),
            },
        ];

        organizations
    }

    #[tokio::test]
    async fn insert() {
        let init = init().await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let orgs = vec![
            Organization {
                id: "".to_string(),
                name: "org1".to_string(),
            },
            Organization {
                id: "".to_string(),
                name: "org2".to_string(),
            },
        ];

        let returned_orgs = store.upsert(orgs.clone()).await.unwrap();

        let mut orgs = orgs
            .into_iter()
            .zip(returned_orgs.iter())
            .map(|(mut org, store_org)| {
                org.id = store_org.id.clone();
                org
            })
            .collect::<Vec<_>>();

        assert_eq!(orgs, returned_orgs);

        let store_org_rows: Vec<OrganizationRow> =
            sqlx::query_as("SELECT id, name FROM organizations")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_orgs: Vec<Organization> = store_org_rows
            .into_iter()
            .map(|row| row.into())
            .collect::<Vec<_>>();

        orgs.sort_by(|a, b| a.id.cmp(&b.id));
        store_orgs.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(orgs, store_orgs);
    }

    #[tokio::test]
    async fn update() {
        let init = init().await;
        let mut orgs = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        orgs[0].name = "org1_updated".to_string();

        let returned_orgs = store.upsert(vec![orgs[0].clone()]).await.unwrap();

        assert_eq!(orgs[0], returned_orgs[0]);

        let store_org_rows: Vec<OrganizationRow> =
            sqlx::query_as("SELECT id, name FROM organizations")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_orgs: Vec<Organization> = store_org_rows
            .into_iter()
            .map(|row| row.into())
            .collect::<Vec<_>>();

        orgs.sort_by(|a, b| a.id.cmp(&b.id));
        store_orgs.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(orgs, store_orgs);
    }

    #[tokio::test]
    async fn update_does_not_exist() {
        let init = init().await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let id = new_id();

        let result = store
            .upsert(vec![Organization {
                id: id.clone(),
                name: "whatever".to_string(),
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
        CompoundQuery,
        NoResults,
    }

    #[test_case(QueryTest::All ; "all")]
    #[test_case(QueryTest::Id ; "id")]
    #[test_case(QueryTest::CompoundQuery ; "compound query")]
    #[test_case(QueryTest::NoResults ; "no results")]
    #[tokio::test]
    async fn query(test_name: QueryTest) {
        let init = init().await;
        let mut orgs = test_data(&init).await;

        struct TestCase {
            query: Option<Query>,
            expected: Vec<Organization>,
        }

        let tc = match test_name {
            QueryTest::All => TestCase {
                query: None,
                expected: orgs,
            },
            QueryTest::Id => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(orgs[0].id.clone()))),
                expected: vec![orgs.remove(0)],
            },
            QueryTest::CompoundQuery => TestCase {
                query: Some(Query::CompoundQuery(CompoundQuery {
                    operator: CompoundOperator::Or,
                    queries: orgs
                        .iter()
                        .map(|e| Query::Id(LogicalQuery::Equals(e.id.clone())))
                        .collect(),
                })),
                expected: orgs,
            },
            QueryTest::NoResults => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(new_id()))),
                expected: vec![],
            },
        };

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let mut returned_orgs = store.query(tc.query.as_ref()).await.unwrap();

        let mut expected_orgs = tc.expected;
        expected_orgs.sort_by(|a, b| a.id.cmp(&b.id));
        returned_orgs.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(expected_orgs, returned_orgs);
    }

    #[tokio::test]
    async fn delete() {
        let init = init().await;
        let orgs = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        store.delete(&[orgs[0].id.clone()]).await.unwrap();

        let store_org_rows: Vec<OrganizationRow> =
            sqlx::query_as("SELECT id, name FROM organizations")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_orgs: Vec<Organization> = store_org_rows
            .into_iter()
            .map(|row| row.into())
            .collect::<Vec<_>>();

        store_orgs.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(orgs[1], store_orgs[0]);
    }
}
