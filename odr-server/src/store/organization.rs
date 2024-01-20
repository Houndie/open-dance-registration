use std::sync::Arc;

use super::{
    common::{ids_in_table, new_id},
    Error,
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

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn upsert(&self, organizations: Vec<Organization>) -> Result<Vec<Organization>, Error>;
    async fn list(&self, ids: Vec<String>) -> Result<Vec<Organization>, Error>;
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
    async fn upsert(&self, organizations: Vec<Organization>) -> Result<Vec<Organization>, Error> {
        let (inserts, updates): (Vec<_>, Vec<_>) = organizations
            .into_iter()
            .enumerate()
            .partition(|(_, org)| org.id == "");

        if !updates.is_empty() {
            // Make sure events exist
            ids_in_table(
                &*self.pool,
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
            .map_err(|e| Error::TransactionStartError(e))?;

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
                .map_err(|e| Error::InsertionError(e))?;
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
                .map_err(|e| Error::UpdateError(e))?;
        };

        tx.commit().await.map_err(|e| Error::TransactionFailed(e))?;

        let mut outputs = Vec::new();
        outputs.resize(inserts.len() + updates.len(), Organization::default());
        inserts
            .into_iter()
            .chain(updates.into_iter())
            .for_each(|(idx, org)| {
                outputs[idx] = org;
            });

        Ok(outputs)
    }
    async fn list(&self, ids: Vec<String>) -> Result<Vec<Organization>, Error> {
        let base_query = "SELECT id, name FROM organizations";

        if ids.is_empty() {
            let organizations: Vec<OrganizationRow> = sqlx::query_as(base_query)
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            return Ok(organizations.into_iter().map(|row| row.into()).collect());
        }

        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("{} WHERE {}", base_query, where_clause);

        let query_builder = sqlx::query_as(&query);
        let query_builder = ids
            .iter()
            .fold(query_builder, |query_builder, id| query_builder.bind(id));

        let rows: Vec<OrganizationRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| Error::FetchError(e))?;

        Ok(rows.into_iter().map(|row| row.into()).collect())
    }
    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }

        ids_in_table(
            &*self.pool,
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
            .map_err(|e| Error::DeleteError(e))?;

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

    use crate::store::{common::new_id, Error};

    use super::{OrganizationRow, SqliteStore, Store};

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

    #[tokio::test]
    async fn list_all() {
        let init = init().await;
        let mut orgs = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let mut returned_orgs = store.list(vec![]).await.unwrap();

        orgs.sort_by(|a, b| a.id.cmp(&b.id));
        returned_orgs.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(orgs, returned_orgs);
    }

    #[tokio::test]
    async fn list_some() {
        let init = init().await;
        let orgs = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let returned_orgs = store.list(vec![orgs[0].id.clone()]).await.unwrap();

        assert_eq!(orgs[0], returned_orgs[0]);
    }

    #[tokio::test]
    async fn delete() {
        let init = init().await;
        let orgs = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        store.delete(&vec![orgs[0].id.clone()]).await.unwrap();

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
