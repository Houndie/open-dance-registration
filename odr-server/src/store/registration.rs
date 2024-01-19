use std::{collections::HashMap, iter, str::FromStr, sync::Arc};

use common::proto::{
    registration_item, Registration, RegistrationItem, RegistrationQuery, RepeatedUint32,
};
use sqlx::SqlitePool;

use super::{
    common::{ids_in_table, new_id},
    Error,
};

#[derive(sqlx::FromRow)]
struct RegistrationRow {
    id: String,
    event: String,
}

impl RegistrationRow {
    fn to_registration(self) -> Registration {
        Registration {
            id: self.id,
            event_id: self.event,
            items: Vec::new(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct RegistrationItemRow {
    id: String,
    registration: String,
    schema_item: String,
    value_type: String,
    value: String,
}

impl RegistrationItemRow {
    fn to_registration_item(self) -> Result<(String, RegistrationItem), Error> {
        let value = match self.value_type.as_str() {
            "StringValue" => registration_item::Value::StringValue(self.value),
            "BooleanValue" => registration_item::Value::BooleanValue(
                bool::from_str(&self.value).map_err(|_| Error::ColumnParseError("value"))?,
            ),
            "UnsignedNumberValue" => registration_item::Value::UnsignedNumberValue(
                u32::from_str(&self.value).map_err(|_| Error::ColumnParseError("value"))?,
            ),
            "RepeatedUnsignedNumberValue" => {
                registration_item::Value::RepeatedUnsignedNumberValue(RepeatedUint32 {
                    value: self
                        .value
                        .split(",")
                        .map(|s| u32::from_str(s))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| Error::ColumnParseError("value"))?,
                })
            }
            _ => return Err(Error::ColumnParseError("value_type")),
        };
        Ok((
            self.registration,
            RegistrationItem {
                id: self.id,
                schema_item_id: self.schema_item,
                value: Some(value),
            },
        ))
    }
}

fn attach_items(
    registrations: impl IntoIterator<Item = Registration>,
    registration_items: impl IntoIterator<Item = (String, RegistrationItem)>,
) -> Vec<Registration> {
    let mut item_map = HashMap::new();
    for (id, item) in registration_items {
        item_map.entry(id).or_insert(Vec::new()).push(item);
    }

    registrations
        .into_iter()
        .map(|mut registration| {
            registration.items = item_map.remove(&registration.id).unwrap_or_default();
            registration
        })
        .collect()
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

type QueryBuilder<'q> = sqlx::query::Query<
    'q,
    sqlx::Sqlite,
    <sqlx::Sqlite as sqlx::database::HasArguments<'q>>::Arguments,
>;

#[tonic::async_trait]
pub trait Store: Send + Sync + 'static {
    async fn upsert(&self, registrations: Vec<Registration>) -> Result<Vec<Registration>, Error>;
    async fn query(&self, query: RegistrationQuery) -> Result<Vec<Registration>, Error>;
    async fn list(&self, ids: Vec<String>) -> Result<Vec<Registration>, Error>;
    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error>;
}

fn bind_item<'q>(
    query_builder: QueryBuilder<'q>,
    registration_id: &'q str,
    item: &'q RegistrationItem,
) -> QueryBuilder<'q> {
    let query_builder = query_builder
        .bind(&item.id)
        .bind(registration_id)
        .bind(&item.schema_item_id);

    let query_builder = match item.value.as_ref().unwrap() {
        registration_item::Value::StringValue(v) => query_builder.bind("StringValue").bind(v),
        registration_item::Value::BooleanValue(v) => {
            query_builder.bind("BooleanValue").bind(v.to_string())
        }
        registration_item::Value::UnsignedNumberValue(v) => query_builder
            .bind("UnsignedNumberValue")
            .bind(v.to_string()),
        registration_item::Value::RepeatedUnsignedNumberValue(v) => {
            query_builder.bind("RepeatedUnsignedNumberValue").bind(
                itertools::Itertools::intersperse(
                    v.value.iter().map(|u| u.to_string()),
                    ",".to_owned(),
                )
                .collect::<String>(),
            )
        }
    };

    query_builder
}

#[tonic::async_trait]
impl Store for SqliteStore {
    async fn upsert(&self, registrations: Vec<Registration>) -> Result<Vec<Registration>, Error> {
        ids_in_table(
            &*self.pool,
            "events",
            registrations
                .iter()
                .map(|registration| registration.event_id.as_str()),
        )
        .await?;

        let registrations_and_items = registrations.into_iter().enumerate().map(|(idx, mut r)| {
            let items = std::mem::take(&mut r.items);
            ((idx, r), items)
        });

        let (inserts_and_items, updates_and_items): (Vec<_>, Vec<_>) =
            registrations_and_items.partition(|((_, r), _)| r.id == "");

        if !updates_and_items.is_empty() {
            ids_in_table(
                &*self.pool,
                "registrations",
                updates_and_items.iter().map(|((_, r), _)| r.id.as_str()),
            )
            .await?;
        }

        let (updates, items_from_updates): (Vec<_>, Vec<_>) = updates_and_items.into_iter().unzip();

        let (insert_items, update_items): (Vec<_>, Vec<_>) = updates
            .iter()
            .map(|(idx, _)| idx)
            .zip(items_from_updates.into_iter())
            .map(|(registration_idx, items)| {
                items
                    .into_iter()
                    .enumerate()
                    .map(|(item_idx, item)| (*registration_idx, item_idx, item))
            })
            .flatten()
            .partition(|(_, _, item)| item.id == "");

        if !update_items.is_empty() {
            ids_in_table(
                &*self.pool,
                "registration_items",
                update_items.iter().map(|(_, _, item)| item.id.as_str()),
            )
            .await?;
        }

        let (inserts, items_from_inserts): (Vec<_>, Vec<_>) = inserts_and_items
            .into_iter()
            .map(|((idx, mut r), items)| {
                r.id = new_id();
                ((idx, r), items)
            })
            .unzip();

        let insert_items = inserts
            .iter()
            .map(|(idx, _)| idx)
            .zip(items_from_inserts.into_iter())
            .map(|(registration_idx, items)| {
                items
                    .into_iter()
                    .enumerate()
                    .map(|(item_idx, item)| (*registration_idx, item_idx, item))
            })
            .flatten()
            .chain(insert_items.into_iter())
            .map(|(registration_idx, item_idx, mut item)| {
                item.id = new_id();
                (registration_idx, item_idx, item)
            })
            .collect::<Vec<_>>();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Error::TransactionStartError(e))?;

        if !inserts.is_empty() {
            let values_clause: String =
                itertools::Itertools::intersperse(inserts.iter().map(|_| "(?, ?)"), ", ").collect();

            let query = format!(
                "INSERT INTO registrations(id, event) VALUES {}",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = inserts.iter().fold(query_builder, |query_builder, (_, r)| {
                query_builder.bind(&r.id).bind(&r.event_id)
            });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::InsertionError(e))?;
        }

        if !updates.is_empty() {
            let values_clause: String =
                itertools::Itertools::intersperse(updates.iter().map(|_| "(?, ?)"), ", ").collect();

            let query = format!(
                "WITH mydata(id, event) AS (VALUES {}) 
                UPDATE registrations 
                SET event = mydata.event 
                FROM mydata 
                WHERE registrations.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = updates.iter().fold(query_builder, |query_builder, (_, r)| {
                query_builder.bind(&r.id).bind(&r.event_id)
            });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;
        };

        let mut outputs = Vec::new();

        inserts
            .into_iter()
            .chain(updates.into_iter())
            .for_each(|(idx, registration)| {
                if outputs.len() <= idx {
                    outputs.resize_with(idx + 1, Default::default);
                };
                outputs[idx] = Some(registration);
            });

        let outputs = outputs
            .into_iter()
            .map(|registration| registration.unwrap())
            .collect::<Vec<_>>();

        if !insert_items.is_empty() {
            let values_clause: String = itertools::Itertools::intersperse(
                insert_items.iter().map(|_| "(?, ?, ?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!(
                "INSERT INTO registration_items(id, registration, schema_item, value_type, value) VALUES {}", values_clause);

            let query_builder = sqlx::query(&query);
            let query_builder = insert_items.iter().fold(
                query_builder,
                |query_builder, (registration_idx, _, item)| {
                    bind_item(query_builder, &outputs[*registration_idx].id, item)
                },
            );
            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::InsertionError(e))?;
        }

        if !update_items.is_empty() {
            let values_clause: String = itertools::Itertools::intersperse(
                update_items.iter().map(|_| "(?, ?, ?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!(
                "WITH mydata(id, registration, schema_item, value_type, value) 
                AS (VALUES {})
                UPDATE registration_items
                SET
                    registration = mydata.registration,
                    schema_item = mydata.schema_item,
                    value_type = mydata.value_type,
                    value = mydata.value
                WHERE registration_items.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = insert_items.iter().fold(
                query_builder,
                |query_builder, (registration_idx, _, item)| {
                    bind_item(query_builder, &outputs[*registration_idx].id, item)
                },
            );
            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;
        }

        let mut items_by_registration = iter::repeat(Vec::new())
            .take(outputs.len())
            .collect::<Vec<_>>();

        insert_items
            .into_iter()
            .chain(update_items.into_iter())
            .for_each(|(registration_idx, item_idx, item)| {
                if items_by_registration[registration_idx].len() <= item_idx {
                    items_by_registration[registration_idx]
                        .resize_with(item_idx + 1, Default::default)
                }

                items_by_registration[registration_idx][item_idx] = Some(item);
            });

        let outputs = outputs
            .into_iter()
            .zip(items_by_registration.into_iter())
            .map(|(mut registration, items)| {
                registration.items = items.into_iter().map(|item| item.unwrap()).collect();
                registration
            })
            .collect::<Vec<_>>();

        {
            let where_clause = itertools::Itertools::intersperse(
                outputs.iter().map(|r| {
                    let registration_clause = itertools::Itertools::intersperse(
                        std::iter::once("registration = ?")
                            .chain(r.items.iter().map(|_| "id != ?")),
                        " AND ",
                    )
                    .collect::<String>();

                    format!("({})", registration_clause)
                }),
                " OR ".to_owned(),
            )
            .collect::<String>();

            let query = format!("DELETE FROM registration_items WHERE {}", where_clause);
            let query_builder = sqlx::query(&query);
            let query_builder = outputs.iter().fold(query_builder, |query_builder, r| {
                let query_builder = query_builder.bind(&r.id);
                r.items.iter().fold(query_builder, |query_builder, item| {
                    query_builder.bind(&item.id)
                })
            });
            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::DeleteError(e))?;
        }

        tx.commit().await.map_err(|e| Error::TransactionFailed(e))?;

        Ok(outputs)
    }
    async fn query(&self, query: RegistrationQuery) -> Result<Vec<Registration>, Error> {
        Ok(Vec::new())
    }
    async fn list(&self, ids: Vec<String>) -> Result<Vec<Registration>, Error> {
        Ok(Vec::new())
    }
    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use common::proto::{registration_item, Registration, RegistrationItem};
    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use super::{attach_items, RegistrationItemRow, RegistrationRow, SqliteStore, Store};
    use crate::store::common::new_id;

    struct Init {
        event_1: String,
        event_2: String,
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

        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, name) VALUES (?, ?), (?, ?);")
            .bind(&id_1)
            .bind(&name_1)
            .bind(&id_2)
            .bind(&name_2)
            .execute(&db)
            .await
            .unwrap();

        Init {
            event_1: id_1,
            event_2: id_2,
            db,
        }
    }

    #[tokio::test]
    async fn insert() {
        let init = init_db().await;
        let registrations = vec![Registration {
            id: "".to_owned(),
            event_id: init.event_1,
            items: vec![RegistrationItem {
                id: "".to_owned(),
                schema_item_id: "schema 1".to_owned(),
                value: Some(registration_item::Value::StringValue("value".to_owned())),
            }],
        }];

        let db = Arc::new(init.db);

        let store = SqliteStore::new(db.clone());

        let returned_registrations = store.upsert(registrations.clone()).await.unwrap();

        let mut registrations = registrations
            .into_iter()
            .zip(returned_registrations.iter())
            .map(|(mut registration, returned_registration)| {
                registration.id = returned_registration.id.clone();
                registration.items = registration
                    .items
                    .into_iter()
                    .zip(returned_registration.items.iter())
                    .map(|(mut item, returned_item)| {
                        item.id = returned_item.id.clone();
                        item
                    })
                    .collect();

                registration
            })
            .collect::<Vec<_>>();

        assert_eq!(registrations, returned_registrations);

        let store_row: Vec<RegistrationRow> = sqlx::query_as("SELECT * FROM registrations")
            .fetch_all(&*db)
            .await
            .unwrap();

        let store_item_row: Vec<RegistrationItemRow> =
            sqlx::query_as("SELECT * FROM registration_items")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_registrations = attach_items(
            store_row.into_iter().map(|row| row.to_registration()),
            store_item_row
                .into_iter()
                .map(|row| row.to_registration_item().unwrap()),
        );

        registrations.sort_by(|l, r| l.id.cmp(&r.id));
        for r in registrations.iter_mut() {
            r.items.sort_by(|l, r| l.id.cmp(&r.id));
        }

        store_registrations.sort_by(|l, r| l.id.cmp(&r.id));
        for r in store_registrations.iter_mut() {
            r.items.sort_by(|l, r| l.id.cmp(&r.id));
        }

        assert_eq!(registrations, store_registrations);
    }
}
