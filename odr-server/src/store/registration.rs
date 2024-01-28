use std::{collections::HashMap, iter, str::FromStr, sync::Arc};

use common::proto::{registration_item, Registration, RegistrationItem, RepeatedUint32};
use sqlx::SqlitePool;

use super::{
    common::{ids_in_table, new_id},
    Bindable as _, Error, Queryable as _,
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

pub struct IdField;

impl super::Field for IdField {
    type Item = String;

    fn field() -> &'static str {
        "id"
    }
}

pub type IdQuery = super::LogicalQuery<IdField>;

pub struct EventIdField;

impl super::Field for EventIdField {
    type Item = String;

    fn field() -> &'static str {
        "event"
    }
}

pub type EventIdQuery = super::LogicalQuery<EventIdField>;

pub enum Query {
    Id(IdQuery),
    EventId(EventIdQuery),
    Compound(super::CompoundQuery<Query>),
}

impl super::Queryable for Query {
    fn where_clause(&self) -> String {
        match self {
            Query::Id(query) => query.where_clause(),
            Query::EventId(query) => query.where_clause(),
            Query::Compound(query) => query.where_clause(),
        }
    }
}

impl<'q, DB: sqlx::Database> super::Bindable<'q, DB> for Query
where
    <IdField as super::Field>::Item: sqlx::Encode<'q, DB> + sqlx::Type<DB> + Sync,
    <EventIdField as super::Field>::Item: sqlx::Encode<'q, DB> + sqlx::Type<DB> + Sync,
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
            Query::Id(query) => query.bind(query_builder),
            Query::EventId(query) => query.bind(query_builder),
            Query::Compound(query) => query.bind(query_builder),
        }
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
    async fn query(&self, query: Query) -> Result<Vec<Registration>, Error>;
    async fn list(&self) -> Result<Vec<Registration>, Error>;
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
            let values_clause: String = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?)").take(inserts.len()),
                ", ",
            )
            .collect();

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
            let values_clause: String = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?)").take(updates.len()),
                ", ",
            )
            .collect();

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
                std::iter::repeat("(?, ?, ?, ?, ?)").take(insert_items.len()),
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
                std::iter::repeat("(?, ?, ?, ?, ?)").take(update_items.len()),
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
                FROM mydata
                WHERE registration_items.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = update_items.iter().fold(
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
                            .chain(std::iter::repeat("id != ?").take(r.items.len())),
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

    async fn query(&self, query: Query) -> Result<Vec<Registration>, Error> {
        let registrations = {
            let query_string = format!(
                "SELECT id, event FROM registrations WHERE {}",
                query.where_clause()
            );
            let query_builder = sqlx::query_as(&query_string);
            let query_builder = query.bind(query_builder);
            let rows: Vec<RegistrationRow> = query_builder
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            rows.into_iter()
                .map(|row| row.to_registration())
                .collect::<Vec<_>>()
        };

        if registrations.is_empty() {
            return Ok(registrations);
        }

        let items = {
            let where_clause: String = itertools::Itertools::intersperse(
                std::iter::repeat("registration = ?").take(registrations.len()),
                " OR ",
            )
            .collect();

            let query = format!(
                "SELECT id, registration, schema_item, value_type, value FROM registration_items WHERE {}",
                where_clause
            );

            let query_builder = sqlx::query_as(&query);
            let query_builder = registrations
                .iter()
                .fold(query_builder, |query_builder, r| query_builder.bind(&r.id));

            let rows: Vec<RegistrationItemRow> = query_builder
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            rows.into_iter()
                .map(|row| row.to_registration_item())
                .collect::<Result<Vec<_>, _>>()?
        };

        let registrations = attach_items(registrations.into_iter(), items.into_iter());

        Ok(registrations)
    }

    async fn list(&self) -> Result<Vec<Registration>, Error> {
        let base_query = "SELECT id, event FROM registrations";
        let base_items_query =
            "SELECT id, registration, schema_item, value_type, value FROM registration_items";

        let registrations: Vec<RegistrationRow> = sqlx::query_as(base_query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| Error::FetchError(e))?;

        let items: Vec<RegistrationItemRow> = sqlx::query_as(base_items_query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| Error::FetchError(e))?;

        let registrations = attach_items(
            registrations.into_iter().map(|row| row.to_registration()),
            items
                .into_iter()
                .map(|row| row.to_registration_item())
                .collect::<Result<Vec<_>, _>>()?,
        );

        Ok(registrations)
    }

    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }

        ids_in_table(
            &*self.pool,
            "registrations",
            ids.iter().map(|id| id.as_str()),
        )
        .await?;

        let where_clause: String =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect();

        let query = format!("DELETE FROM registrations WHERE {}", where_clause);
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

    use common::proto::{registration_item, Registration, RegistrationItem, RepeatedUint32};
    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use super::{attach_items, RegistrationItemRow, RegistrationRow, SqliteStore, Store};
    use crate::store::{
        common::new_id, registration::Query, CompoundOperator, CompoundQuery, Error, LogicalQuery,
    };
    use test_case::test_case;

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

        let org_id = new_id();
        let org_name = "Org 1";
        sqlx::query("INSERT INTO organizations(id, name) VALUES (?, ?);")
            .bind(&org_id)
            .bind(&org_name)
            .execute(&db)
            .await
            .unwrap();

        let id_1 = new_id();
        let name_1 = "Event 1";
        let id_2 = new_id();
        let name_2 = "Event 2";
        sqlx::query("INSERT INTO events(id, organization, name) VALUES (?, ?, ?), (?, ?, ?);")
            .bind(&id_1)
            .bind(&org_id)
            .bind(&name_1)
            .bind(&id_2)
            .bind(&org_id)
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

    async fn test_data(init: &Init) -> Vec<Registration> {
        let registration1_id = new_id();
        let item1_id = new_id();
        let item1_schema_item_id = "schema 1".to_owned();
        let item1_value = "value";
        let item2_id = new_id();
        let item2_schema_item_id = "schema 2".to_owned();
        let item2_value = true;
        let registration2_id = new_id();
        let item3_id = new_id();
        let item3_schema_item_id = "schema 3".to_owned();
        let item3_value: u32 = 1;
        let item4_id = new_id();
        let item4_schema_item_id = "schema 4".to_owned();
        let item4_value: Vec<u32> = vec![1, 2, 3];

        {
            let query = sqlx::query("INSERT INTO registrations(id, event) VALUES (?, ?), (?, ?);");

            let query = query
                .bind(&registration1_id)
                .bind(&init.event_1)
                .bind(&registration2_id)
                .bind(&init.event_2);

            query.execute(&init.db).await.unwrap();
        }

        {
            let query = sqlx::query(
                "INSERT INTO registration_items(id, registration, schema_item, value_type, value) VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?);",
            );

            let query = query
                .bind(&item1_id)
                .bind(&registration1_id)
                .bind(&item1_schema_item_id)
                .bind("StringValue")
                .bind(&item1_value)
                .bind(&item2_id)
                .bind(&registration1_id)
                .bind(&item2_schema_item_id)
                .bind("BooleanValue")
                .bind(item2_value.to_string())
                .bind(&item3_id)
                .bind(&registration2_id)
                .bind(&item3_schema_item_id)
                .bind("UnsignedNumberValue")
                .bind(item3_value.to_string())
                .bind(&item4_id)
                .bind(&registration2_id)
                .bind(&item4_schema_item_id)
                .bind("RepeatedUnsignedNumberValue")
                .bind(
                    itertools::Itertools::intersperse(
                        item4_value.iter().map(|u| u.to_string()),
                        ",".to_owned(),
                    )
                    .collect::<String>(),
                );

            query.execute(&init.db).await.unwrap();
        }

        let registrations = vec![
            Registration {
                id: registration1_id,
                event_id: init.event_1.clone(),
                items: vec![
                    RegistrationItem {
                        id: item1_id,
                        schema_item_id: item1_schema_item_id,
                        value: Some(registration_item::Value::StringValue(
                            item1_value.to_owned(),
                        )),
                    },
                    RegistrationItem {
                        id: item2_id,
                        schema_item_id: item2_schema_item_id,
                        value: Some(registration_item::Value::BooleanValue(item2_value)),
                    },
                ],
            },
            Registration {
                id: registration2_id,
                event_id: init.event_2.clone(),
                items: vec![
                    RegistrationItem {
                        id: item3_id,
                        schema_item_id: item3_schema_item_id,
                        value: Some(registration_item::Value::UnsignedNumberValue(item3_value)),
                    },
                    RegistrationItem {
                        id: item4_id,
                        schema_item_id: item4_schema_item_id,
                        value: Some(registration_item::Value::RepeatedUnsignedNumberValue(
                            RepeatedUint32 { value: item4_value },
                        )),
                    },
                ],
            },
        ];

        registrations
    }

    fn sort_registrations(mut registrations: Vec<Registration>) -> Vec<Registration> {
        registrations.sort_by(|l, r| l.id.cmp(&r.id));
        for r in registrations.iter_mut() {
            r.items.sort_by(|l, r| l.id.cmp(&r.id));
        }

        registrations
    }

    #[tokio::test]
    async fn insert() {
        let init = init_db().await;
        let registrations = vec![
            Registration {
                id: "".to_owned(),
                event_id: init.event_1,
                items: vec![
                    RegistrationItem {
                        id: "".to_owned(),
                        schema_item_id: "schema 1".to_owned(),
                        value: Some(registration_item::Value::StringValue("value".to_owned())),
                    },
                    RegistrationItem {
                        id: "".to_owned(),
                        schema_item_id: "schema 2".to_owned(),
                        value: Some(registration_item::Value::BooleanValue(true)),
                    },
                ],
            },
            Registration {
                id: "".to_owned(),
                event_id: init.event_2,
                items: vec![
                    RegistrationItem {
                        id: "".to_owned(),
                        schema_item_id: "schema 3".to_owned(),
                        value: Some(registration_item::Value::UnsignedNumberValue(1)),
                    },
                    RegistrationItem {
                        id: "".to_owned(),
                        schema_item_id: "schema 4".to_owned(),
                        value: Some(registration_item::Value::RepeatedUnsignedNumberValue(
                            RepeatedUint32 {
                                value: vec![1, 2, 3],
                            },
                        )),
                    },
                ],
            },
        ];

        let db = Arc::new(init.db);

        let store = SqliteStore::new(db.clone());

        let returned_registrations = store.upsert(registrations.clone()).await.unwrap();

        let registrations = registrations
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

        let store_registrations = attach_items(
            store_row.into_iter().map(|row| row.to_registration()),
            store_item_row
                .into_iter()
                .map(|row| row.to_registration_item().unwrap()),
        );

        let registrations = sort_registrations(registrations);
        let store_registrations = sort_registrations(store_registrations);

        assert_eq!(registrations, store_registrations);
    }

    #[tokio::test]
    async fn update() {
        let init = init_db().await;
        let mut registrations = test_data(&init).await;
        registrations[0].items[0].value = Some(registration_item::Value::StringValue(
            "updated value".to_owned(),
        ));
        registrations[1].items[1] = RegistrationItem {
            id: "".to_owned(),
            schema_item_id: "schema 5".to_owned(),
            value: Some(registration_item::Value::UnsignedNumberValue(2)),
        };

        let db = Arc::new(init.db);

        let store = SqliteStore::new(db.clone());

        let returned_registrations = store.upsert(registrations.clone()).await.unwrap();
        registrations[1].items[1].id = returned_registrations[1].items[1].id.clone();

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

        let store_registrations = attach_items(
            store_row.into_iter().map(|row| row.to_registration()),
            store_item_row
                .into_iter()
                .map(|row| row.to_registration_item().unwrap()),
        );

        let registrations = sort_registrations(registrations);
        let store_registrations = sort_registrations(store_registrations);

        assert_eq!(registrations, store_registrations);
    }

    enum UpdateDoesNotExistTests {
        BadRegistrationId,
        BadEventId,
        BadItemId,
    }
    #[test_case(UpdateDoesNotExistTests::BadRegistrationId ; "bad registration id")]
    #[test_case(UpdateDoesNotExistTests::BadEventId ; "bad event id")]
    #[test_case(UpdateDoesNotExistTests::BadItemId ; "bad item id")]
    #[tokio::test]
    async fn update_does_not_exist(test_name: UpdateDoesNotExistTests) {
        let init = init_db().await;
        let test_data = test_data(&init).await;

        struct TestCase {
            id: String,
            registration: Registration,
        }
        let tc = match test_name {
            UpdateDoesNotExistTests::BadRegistrationId => {
                let id = new_id();
                TestCase {
                    id: id.clone(),
                    registration: Registration {
                        id,
                        event_id: init.event_1,
                        items: Vec::new(),
                    },
                }
            }

            UpdateDoesNotExistTests::BadEventId => {
                let id = new_id();
                let mut registration = test_data[0].clone();
                registration.event_id = id.clone();
                TestCase { id, registration }
            }
            UpdateDoesNotExistTests::BadItemId => {
                let id = new_id();
                let mut registration = test_data[0].clone();
                registration.items[0].id = id.clone();
                TestCase { id, registration }
            }
        };

        let store = SqliteStore::new(Arc::new(init.db));
        let result = store.upsert(vec![tc.registration]).await;
        match result {
            Ok(_) => panic!("Expected error"),
            Err(Error::IdDoesNotExist(id)) => assert_eq!(id, tc.id),
            _ => panic!("Expected IdDoesNotExistError"),
        }
    }

    #[tokio::test]
    async fn list_all() {
        let init = init_db().await;
        let registrations = test_data(&init).await;

        let store = SqliteStore::new(Arc::new(init.db));
        let returned_registrations = store.list().await.unwrap();

        let registrations = sort_registrations(registrations);
        let returned_registrations = sort_registrations(returned_registrations);

        assert_eq!(registrations, returned_registrations);
    }

    enum QueryTest {
        Id,
        EventId,
        CompoundQuery,
        NoResults,
    }

    #[test_case(QueryTest::Id ; "id")]
    #[test_case(QueryTest::EventId ; "event id")]
    #[test_case(QueryTest::CompoundQuery ; "compound query")]
    #[test_case(QueryTest::NoResults ; "no results")]
    #[tokio::test]
    async fn list_some(test_name: QueryTest) {
        let init = init_db().await;
        let mut registrations = test_data(&init).await;

        struct TestCase {
            query: Query,
            expected: Vec<Registration>,
        }

        let tc = match test_name {
            QueryTest::Id => TestCase {
                query: Query::Id(LogicalQuery::Equals(registrations[0].id.clone())),
                expected: vec![registrations.remove(0)],
            },
            QueryTest::EventId => TestCase {
                query: Query::EventId(LogicalQuery::Equals(init.event_1.clone())),
                expected: vec![registrations.remove(0)],
            },
            QueryTest::CompoundQuery => TestCase {
                query: Query::Compound(CompoundQuery {
                    operator: CompoundOperator::Or,
                    queries: registrations
                        .iter()
                        .map(|r| Query::Id(LogicalQuery::Equals(r.id.clone())))
                        .collect(),
                }),
                expected: registrations,
            },
            QueryTest::NoResults => TestCase {
                query: Query::Id(LogicalQuery::Equals(new_id())),
                expected: Vec::new(),
            },
        };

        let store = SqliteStore::new(Arc::new(init.db));
        let returned_registrations = store.query(tc.query).await.unwrap();

        let returned_registrations = sort_registrations(returned_registrations);
        let expected = sort_registrations(tc.expected);

        assert_eq!(expected, returned_registrations);
    }

    #[tokio::test]
    async fn delete_one() {
        let init = init_db().await;
        let mut registrations = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());
        store
            .delete(&vec![registrations[0].id.clone()])
            .await
            .unwrap();

        registrations.remove(0);

        let store_row: Vec<RegistrationRow> = sqlx::query_as("SELECT * FROM registrations")
            .fetch_all(&*db)
            .await
            .unwrap();

        let store_item_row: Vec<RegistrationItemRow> =
            sqlx::query_as("SELECT * FROM registration_items")
                .fetch_all(&*db)
                .await
                .unwrap();

        let store_registrations = attach_items(
            store_row.into_iter().map(|row| row.to_registration()),
            store_item_row
                .into_iter()
                .map(|row| row.to_registration_item().unwrap()),
        );

        let registrations = sort_registrations(registrations);
        let store_registrations = sort_registrations(store_registrations);

        assert_eq!(registrations, store_registrations);
    }
}
