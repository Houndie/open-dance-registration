use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use sqlx::SqlitePool;

use common::proto::{
    multi_select_type, registration_schema_item_type::Type as ItemType, select_type, text_type,
    CheckboxType, MultiSelectType, RegistrationSchema, RegistrationSchemaItem,
    RegistrationSchemaItemType, SelectOption, SelectType, TextType,
};

use super::{
    common::{ids_in_table, new_id},
    Error,
};

#[derive(sqlx::FromRow)]
struct OptionRow {
    id: String,
    schema_item: String,
    idx: i32,
    name: String,
    product_id: String,
}

impl OptionRow {
    fn to_option(self) -> Result<(String, usize, SelectOption), Error> {
        Ok((
            self.schema_item,
            usize::try_from(self.idx).map_err(|_| Error::ColumnParseError("idx"))?,
            SelectOption {
                id: self.id,
                name: self.name,
                product_id: self.product_id,
            },
        ))
    }
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    id: String,
    event: String,
    idx: i32,
    name: String,
    item_type: String,
    text_type_default: Option<String>,
    text_type_display: Option<String>,
    checkbox_type_default: Option<i32>,
    select_type_default: Option<i32>,
    select_type_display: Option<String>,
    multi_select_type_defaults: Option<String>,
    multi_select_type_display: Option<String>,
}

impl ItemRow {
    fn to_item(self) -> Result<(String, usize, RegistrationSchemaItem), Error> {
        let typ = match self.item_type.as_str() {
            "TextType" => Some(ItemType::Text(TextType {
                default: self
                    .text_type_default
                    .ok_or(Error::ColumnParseError("text_type_default"))?,
                display: text_type::Display::from_str_name(
                    &self
                        .text_type_display
                        .ok_or(Error::ColumnParseError("text_type_display"))?,
                )
                .ok_or(Error::ColumnParseError("text_type_display"))?
                    as i32,
            })),
            "CheckboxType" => Some(ItemType::Checkbox(CheckboxType {
                default: self
                    .checkbox_type_default
                    .ok_or(Error::ColumnParseError("checkbox_type_default"))?
                    != 0,
            })),
            "SelectType" => Some(ItemType::Select(SelectType {
                default: u32::try_from(
                    self.select_type_default
                        .ok_or(Error::ColumnParseError("select_type_default"))?,
                )
                .map_err(|_| Error::ColumnParseError("select_type_default"))?,
                display: select_type::Display::from_str_name(
                    &self
                        .select_type_display
                        .ok_or(Error::ColumnParseError("select_type_display"))?,
                )
                .ok_or(Error::ColumnParseError("select_type_display"))?
                    as i32,
                options: Vec::new(),
            })),
            "MultiSelectType" => {
                let defaults = self
                    .multi_select_type_defaults
                    .ok_or(Error::ColumnParseError("multi_select_type_defaults"))?
                    .split(',')
                    .map(|s| {
                        s.parse::<u32>()
                            .map_err(|_| Error::ColumnParseError("multi_select_type_defaults"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Some(ItemType::MultiSelect(MultiSelectType {
                    defaults,
                    display: multi_select_type::Display::from_str_name(
                        &self
                            .multi_select_type_display
                            .ok_or(Error::ColumnParseError("multi_select_type_defaults"))?,
                    )
                    .ok_or(Error::ColumnParseError("multi_select_type_defaults"))?
                        as i32,
                    options: Vec::new(),
                }))
            }
            _ => None,
        };

        Ok((
            self.event,
            usize::try_from(self.idx).map_err(|_| Error::ColumnParseError("idx"))?,
            RegistrationSchemaItem {
                id: self.id,
                name: self.name,
                r#type: Some(RegistrationSchemaItemType { r#type: typ }),
            },
        ))
    }
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
    async fn upsert(
        &self,
        schemas: Vec<crate::proto::RegistrationSchema>,
    ) -> Result<Vec<RegistrationSchema>, Error>;
    async fn list(&self, ids: Vec<String>) -> Result<Vec<RegistrationSchema>, Error>;
    async fn delete(&self, ids: &Vec<String>) -> Result<(), Error>;
}

fn option_values_bind<'q>(
    query_builder: QueryBuilder<'q>,
    item_id: &'q str,
    idx: usize,
    option: &'q SelectOption,
) -> Result<QueryBuilder<'q>, Error> {
    Ok(query_builder
        .bind(&option.id)
        .bind(item_id)
        .bind(i32::try_from(idx).unwrap())
        .bind(&option.name)
        .bind(&option.product_id))
}

fn values_bind<'q>(
    query_builder: QueryBuilder<'q>,
    event_id: &'q str,
    idx: usize,
    item: &'q RegistrationSchemaItem,
) -> Result<QueryBuilder<'q>, Error> {
    let query_builder = query_builder
        .bind(&item.id)
        .bind(event_id)
        .bind(i32::try_from(idx).unwrap())
        .bind(&item.name);

    let typ = item.r#type.as_ref().unwrap().r#type.as_ref().unwrap();

    let query_builder = match typ {
        ItemType::Text(_) => query_builder.bind("TextType"),
        ItemType::Checkbox(_) => query_builder.bind("CheckboxType"),
        ItemType::Select(_) => query_builder.bind("SelectType"),
        ItemType::MultiSelect(_) => query_builder.bind("MultiSelectType"),
    };

    let query_builder = match typ {
        ItemType::Text(text) => query_builder.bind(&text.default).bind(
            text_type::Display::try_from(text.display)
                .unwrap()
                .as_str_name(),
        ),
        _ => query_builder
            .bind::<Option<String>>(None)
            .bind::<Option<String>>(None),
    };

    let query_builder = match typ {
        ItemType::Checkbox(checkbox) => query_builder.bind(checkbox.default as i32),
        _ => query_builder.bind::<Option<i32>>(None),
    };

    let query_builder = match typ {
        ItemType::Select(select) => query_builder
            .bind(i32::try_from(select.default).unwrap())
            .bind(
                select_type::Display::try_from(select.display)
                    .unwrap()
                    .as_str_name(),
            ),
        _ => query_builder
            .bind::<Option<i32>>(None)
            .bind::<Option<String>>(None),
    };

    let query_builder = match typ {
        ItemType::MultiSelect(select) => {
            let defaults: String = itertools::Itertools::intersperse(
                select.defaults.iter().map(|idx| format!("{}", idx)),
                ",".to_owned(),
            )
            .collect();

            query_builder.bind(defaults).bind(
                multi_select_type::Display::try_from(select.display)
                    .unwrap()
                    .as_str_name(),
            )
        }
        _ => query_builder
            .bind::<Option<String>>(None)
            .bind::<Option<String>>(None),
    };

    Ok(query_builder)
}

fn build_items_map(
    items: impl IntoIterator<Item = (String, usize, RegistrationSchemaItem)>,
    options: impl IntoIterator<Item = (String, usize, SelectOption)>,
) -> HashMap<String, Vec<RegistrationSchemaItem>> {
    let mut items_to_options_map = HashMap::new();
    for (item_id, idx, option) in options {
        let option_map = items_to_options_map
            .entry(item_id)
            .or_insert(BTreeMap::new());
        option_map.insert(idx, option);
    }

    let mut items_to_options_map = items_to_options_map
        .into_iter()
        .map(|(item_id, options_map)| (item_id, options_map.into_values().collect::<Vec<_>>()))
        .collect::<HashMap<_, _>>();

    let mut schema_map = HashMap::new();
    for (event_id, idx, mut item) in items {
        match item.r#type.as_mut().unwrap().r#type.as_mut().unwrap() {
            ItemType::Select(select) => {
                select.options = items_to_options_map.remove(&item.id).unwrap_or_default()
            }
            ItemType::MultiSelect(select) => {
                select.options = items_to_options_map.remove(&item.id).unwrap_or_default()
            }
            _ => (),
        };

        let item_map = schema_map.entry(event_id).or_insert(BTreeMap::new());
        item_map.insert(idx, item);
    }

    schema_map
        .into_iter()
        .map(|(event_id, item_map)| (event_id, item_map.into_values().collect()))
        .collect()
}

fn items_to_schema(
    items: impl IntoIterator<Item = (String, usize, RegistrationSchemaItem)>,
    options: impl IntoIterator<Item = (String, usize, SelectOption)>,
) -> Vec<RegistrationSchema> {
    let schema_map = build_items_map(items, options);

    schema_map
        .into_iter()
        .map(|(event_id, items)| RegistrationSchema { event_id, items })
        .collect()
}

#[tonic::async_trait]
impl Store for SqliteStore {
    async fn upsert(
        &self,
        schemas: Vec<RegistrationSchema>,
    ) -> Result<Vec<RegistrationSchema>, Error> {
        ids_in_table(
            &*self.pool,
            "events",
            schemas.iter().map(|schema| schema.event_id.as_str()),
        )
        .await?;

        let (inserts, updates): (Vec<_>, Vec<_>) = schemas
            .into_iter()
            .map(|s| {
                s.items
                    .into_iter()
                    .enumerate()
                    .map(move |(item_idx, item)| (s.event_id.clone(), item_idx, item))
            })
            .flatten()
            .partition(|(_, _, item)| item.id == "");

        if !updates.is_empty() {
            // Make sure events exist
            ids_in_table(
                &*self.pool,
                "registration_schema_items",
                updates.iter().map(|(_, _, item)| item.id.as_str()),
            )
            .await?;
        }

        let (updates, options_from_updates): (Vec<_>, Vec<_>) = updates
            .into_iter()
            .map(|mut item| {
                let (_, _, i) = &mut item;

                let options = match i.r#type.as_mut().unwrap().r#type.as_mut().unwrap() {
                    ItemType::Select(ref mut select) => std::mem::take(&mut select.options),
                    ItemType::MultiSelect(multi_select) => {
                        std::mem::take(&mut multi_select.options)
                    }
                    _ => Vec::new(),
                };

                let options = options
                    .into_iter()
                    .enumerate()
                    .map(|(idx, option)| (i.id.clone(), idx, option))
                    .collect::<Vec<_>>();

                (item, options)
            })
            .unzip();

        let (insert_options, update_options): (Vec<_>, Vec<_>) = options_from_updates
            .into_iter()
            .flatten()
            .partition(|(_, _, option)| option.id == "");

        if !update_options.is_empty() {
            ids_in_table(
                &*self.pool,
                "registration_schema_select_options",
                update_options
                    .iter()
                    .map(|(_, _, option)| option.id.as_str()),
            )
            .await?;
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Error::TransactionStartError(e))?;

        let inserted = if !inserts.is_empty() {
            let items_with_ids = inserts
                .into_iter()
                .map(|(event_id, item_idx, mut item)| {
                    item.id = new_id();
                    (event_id, item_idx, item)
                })
                .collect::<Vec<_>>();

            let values_clause: String = itertools::Itertools::intersperse(
                items_with_ids
                    .iter()
                    .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!(
                "INSERT INTO registration_schema_items(
                    id, 
                    event, 
                    idx, 
                    name, 
                    item_type, 
                    text_type_default, 
                    text_type_display, 
                    checkbox_type_default, 
                    select_type_default, 
                    select_type_display, 
                    multi_select_type_defaults, 
                    multi_select_type_display
                ) VALUES {}",
                values_clause
            );

            let mut query_builder = sqlx::query(&query);
            for (event_id, item_idx, item) in items_with_ids.iter() {
                query_builder = values_bind(query_builder, event_id, *item_idx, item)?;
            }

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::InsertionError(e))?;

            items_with_ids
        } else {
            Vec::new()
        };

        if !updates.is_empty() {
            let values_clause: String = itertools::Itertools::intersperse(
                updates
                    .iter()
                    .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!(
                "WITH mydata(
                    id, 
                    event,
                    idx,
                    name,
                    item_type,
                    text_type_default,
                    text_type_display,
                    checkbox_type_default,
                    select_type_default,
                    select_type_display,
                    multi_select_type_defaults,
                    multi_select_type_display
                ) AS (VALUES {}) UPDATE registration_schema_items SET 
                    event = mydata.event,
                    name = mydata.name,
                    idx = mydata.idx,
                    item_type = mydata.item_type,
                    text_type_default = mydata.text_type_default,
                    text_type_display = mydata.text_type_display,
                    checkbox_type_default = mydata.checkbox_type_default,
                    select_type_default = mydata.select_type_default,
                    multi_select_type_defaults = mydata.multi_select_type_defaults,
                    multi_select_type_display = mydata.multi_select_type_display
                FROM mydata WHERE registration_schema_items.id = mydata.id",
                values_clause
            );

            let mut query_builder = sqlx::query(&query);
            for (event_id, idx, item) in updates.iter() {
                query_builder = values_bind(query_builder, event_id, *idx, item)?;
            }

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;
        }

        let (inserted, options_from_inserts): (Vec<_>, Vec<_>) = inserted
            .into_iter()
            .map(|mut item| {
                let (_, _, i) = &mut item;

                let options = match i.r#type.as_mut().unwrap().r#type.as_mut().unwrap() {
                    ItemType::Select(select) => std::mem::take(&mut select.options),
                    ItemType::MultiSelect(multi_select) => {
                        std::mem::take(&mut multi_select.options)
                    }
                    _ => Vec::new(),
                };

                let options = options
                    .into_iter()
                    .enumerate()
                    .map(|(idx, option)| (i.id.clone(), idx, option))
                    .collect::<Vec<_>>();

                (item, options)
            })
            .unzip();

        let insert_options = options_from_inserts
            .into_iter()
            .flatten()
            .chain(insert_options.into_iter())
            .collect::<Vec<_>>();

        let inserted_options = if !insert_options.is_empty() {
            let options_with_ids = insert_options
                .into_iter()
                .map(|(item_id, idx, mut option)| {
                    option.id = new_id();
                    (item_id, idx, option)
                })
                .collect::<Vec<_>>();

            let values_clause: String = itertools::Itertools::intersperse(
                options_with_ids.iter().map(|_| "(?, ?, ?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!("INSERT INTO registration_schema_select_options(id, schema_item, idx, name, product_id) VALUES {};", values_clause);

            let mut query_builder = sqlx::query(&query);
            for (item_id, idx, option) in options_with_ids.iter() {
                query_builder = option_values_bind(query_builder, item_id, *idx, option)?;
            }

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::InsertionError(e))?;

            options_with_ids
        } else {
            Vec::new()
        };

        if !update_options.is_empty() {
            let values_clause: String = itertools::Itertools::intersperse(
                update_options.iter().map(|_| "(?, ?, ?, ?, ?)"),
                ", ",
            )
            .collect();

            let query = format!(
                "WITH mydata(id,
                    schema_item,
                    idx,
                    name,
                    product_id
                ) AS (VALUES {}) UPDATE registration_schema_select_options SET 
                    schema_item = mydata.schema_item,
                    idx = mydata.idx,
                    name = mydata.name,
                    product_id = mydata.product_id
                FROM mydata WHERE registration_schema_select_options.id = mydata.id",
                values_clause
            );

            let mut query_builder = sqlx::query(&query);
            for (item_id, idx, option) in update_options.iter() {
                query_builder = option_values_bind(query_builder, item_id, *idx, option)?;
            }

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::UpdateError(e))?;
        }

        let schema = items_to_schema(
            inserted.into_iter().chain(updates.into_iter()),
            inserted_options
                .into_iter()
                .chain(update_options.into_iter()),
        );

        if !schema.is_empty() {
            let where_clause = itertools::Itertools::intersperse(
                schema.iter().map(|schema| {
                    let event_clause = itertools::Itertools::intersperse(
                        std::iter::once("event = ?").chain(schema.items.iter().map(|_| "id != ?")),
                        " AND ",
                    )
                    .collect::<String>();

                    format!("({})", event_clause)
                }),
                " OR ".to_owned(),
            )
            .collect::<String>();

            let query = format!(
                "DELETE FROM registration_schema_items WHERE {}",
                where_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = schema.iter().fold(query_builder, |query_builder, schema| {
                let query_builder = query_builder.bind(&schema.event_id);
                schema
                    .items
                    .iter()
                    .fold(query_builder, |query_builder, item| {
                        query_builder.bind(&item.id)
                    })
            });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::DeleteError(e))?;

            let options_where_clause = itertools::Itertools::intersperse(
                schema
                    .iter()
                    .map(|schema| schema.items.iter())
                    .flatten()
                    .map(|item| {
                        let options: Box<dyn Iterator<Item = &SelectOption>> =
                            match item.r#type.as_ref().unwrap().r#type.as_ref().unwrap() {
                                ItemType::Select(select) => Box::new(select.options.iter()),
                                ItemType::MultiSelect(select) => Box::new(select.options.iter()),
                                _ => Box::new(std::iter::empty()),
                            };

                        itertools::Itertools::intersperse(
                            std::iter::once("schema_item = ?").chain(options.map(|_| "id != ?")),
                            " AND ",
                        )
                        .collect::<String>()
                    }),
                " OR ".to_owned(),
            )
            .collect::<String>();

            let options_query = format!(
                "DELETE FROM registration_schema_select_options WHERE {}",
                options_where_clause
            );

            let query_builder = sqlx::query(&options_query);
            let query_builder = schema
                .iter()
                .map(|schema| schema.items.iter())
                .flatten()
                .fold(query_builder, |query_builder, item| {
                    let query_builder = query_builder.bind(&item.id);

                    let options: Box<dyn Iterator<Item = &SelectOption>> =
                        match item.r#type.as_ref().unwrap().r#type.as_ref().unwrap() {
                            ItemType::Select(select) => Box::new(select.options.iter()),
                            ItemType::MultiSelect(select) => Box::new(select.options.iter()),
                            _ => Box::new(std::iter::empty()),
                        };

                    options.fold(query_builder, |query_builder, option| {
                        query_builder.bind(&option.id)
                    })
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(|e| Error::DeleteError(e))?;
        };

        tx.commit().await.map_err(|e| Error::TransactionFailed(e))?;

        Ok(schema)
    }

    async fn list(&self, ids: Vec<String>) -> Result<Vec<RegistrationSchema>, Error> {
        let base_query = "SELECT id, 
            event,
            idx,
            name,
            item_type,
            text_type_default,
            text_type_display,
            checkbox_type_default,
            select_type_default,
            select_type_display,
            multi_select_type_defaults,
            multi_select_type_display FROM registration_schema_items";

        let base_options_query =
            "SELECT id, schema_item, idx, name, product_id FROM registration_schema_select_options";

        if ids.is_empty() {
            let item_rows: Vec<ItemRow> = sqlx::query_as(base_query)
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            let items = item_rows
                .into_iter()
                .map(|row| row.to_item())
                .collect::<Result<Vec<_>, _>>()?;

            let option_rows: Vec<OptionRow> = sqlx::query_as(base_options_query)
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            let options = option_rows
                .into_iter()
                .map(|row| row.to_option())
                .collect::<Result<Vec<_>, _>>()?;

            return Ok(items_to_schema(items, options));
        }

        ids_in_table(&*self.pool, "events", ids.iter().map(|id| id.as_str())).await?;

        let items = {
            let where_clause: String =
                itertools::Itertools::intersperse(ids.iter().map(|_| "event = ?"), " OR ")
                    .collect();
            let query = format!("{} WHERE {}", base_query, where_clause);

            let query_builder = sqlx::query_as(&query);
            let query_builder = ids
                .iter()
                .fold(query_builder, |query_builder, id| query_builder.bind(id));

            let rows: Vec<ItemRow> = query_builder
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            rows.into_iter()
                .map(|row| row.to_item())
                .collect::<Result<Vec<_>, _>>()?
        };

        let options = {
            let select_items = items
                .iter()
                .filter(|(_, _, item)| {
                    match item.r#type.as_ref().unwrap().r#type.as_ref().unwrap() {
                        ItemType::Select(_) | ItemType::MultiSelect(_) => true,
                        _ => false,
                    }
                })
                .collect::<Vec<_>>();

            let where_clause: String = itertools::Itertools::intersperse(
                select_items.iter().map(|_| "schema_item = ?"),
                " OR ",
            )
            .collect();

            let query = format!("{} WHERE {}", base_options_query, where_clause);

            let query_builder = sqlx::query_as(&query);
            let query_builder = select_items
                .iter()
                .fold(query_builder, |query_builder, (_, _, item)| {
                    query_builder.bind(&item.id)
                });

            let rows: Vec<OptionRow> = query_builder
                .fetch_all(&*self.pool)
                .await
                .map_err(|e| Error::FetchError(e))?;

            rows.into_iter()
                .map(|row| row.to_option())
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut schema_map = build_items_map(items, options);

        let schemas = ids
            .into_iter()
            .map(|id| {
                let items = schema_map.remove(&id).unwrap_or_default();
                RegistrationSchema {
                    event_id: id,
                    items,
                }
            })
            .collect();

        Ok(schemas)
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
            itertools::Itertools::intersperse(event_ids.iter().map(|_| "event = ?"), " OR ")
                .collect();
        let query = format!(
            "DELETE FROM registration_schema_items WHERE {}",
            where_clause
        );

        let mut query_builder = sqlx::query(&query);

        for id in event_ids.iter() {
            query_builder = query_builder.bind(id);
        }

        query_builder
            .execute(&*self.pool)
            .await
            .map_err(|e| Error::DeleteError(e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr, sync::Arc};

    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };

    use crate::{
        proto::{
            multi_select_type, registration_schema_item_type::Type as ItemType, select_type,
            text_type, CheckboxType, MultiSelectType, RegistrationSchema, RegistrationSchemaItem,
            RegistrationSchemaItemType, SelectOption, SelectType, TextType,
        },
        store::{
            common::new_id,
            registration_schema::{OptionRow, Store},
            Error,
        },
    };

    use super::{items_to_schema, ItemRow, SqliteStore};

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

    async fn test_data(init: &Init) -> Vec<RegistrationSchema> {
        let item1_id = new_id();
        let item1_name = "item 1";
        let item2_id = new_id();
        let item2_name = "item 2";
        let item3_id = new_id();
        let item3_name = "item 3";
        let item4_id = new_id();
        let item4_name = "item 4";
        let text_default = "text default";
        let text_display = ("LARGE", text_type::Display::Large);
        let checkbox_default = true;
        let select_default = 1;
        let select_display = ("DROPDOWN", select_type::Display::Dropdown);
        let select_option1_id = new_id();
        let select_option1_name = "option 1";
        let select_option1_product_id = "product 1";
        let select_option2_id = new_id();
        let select_option2_name = "option 2";
        let select_option2_product_id = "product 2";
        let select_option3_id = new_id();
        let select_option3_name = "option 3";
        let select_option3_product_id = "product 3";
        let text_default_4 = "text default";
        let text_display_4 = ("SMALL", text_type::Display::Small);

        {
            let query = sqlx::query(
                "INSERT INTO registration_schema_items(id, 
                    event, 
                    idx, 
                    name, 
                    item_type, 
                    text_type_default, 
                    text_type_display, 
                    checkbox_type_default, 
                    select_type_default,
                    select_type_display, 
                    multi_select_type_defaults, 
                    multi_select_type_display 
                ) VALUES 
                    (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?),
                    (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?),
                    (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?),
                    (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            );

            // item 1
            let query = query
                .bind(&item1_id)
                .bind(&init.event_1)
                .bind(0)
                .bind(item1_name)
                .bind("TextType")
                .bind(text_default)
                .bind(text_display.0)
                .bind::<Option<i32>>(None)
                .bind::<Option<i32>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None);

            // item 2
            let query = query
                .bind(&item2_id)
                .bind(&init.event_1)
                .bind(1)
                .bind(item2_name)
                .bind("CheckboxType")
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None)
                .bind(checkbox_default as i32)
                .bind::<Option<i32>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None);

            // item 3
            let query = query
                .bind(&item3_id)
                .bind(&init.event_1)
                .bind(2)
                .bind(item3_name)
                .bind("SelectType")
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<i32>>(None)
                .bind(select_default)
                .bind(select_display.0)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None);

            // item 4
            let query = query
                .bind(&item4_id)
                .bind(&init.event_2)
                .bind(0)
                .bind(item4_name)
                .bind("TextType")
                .bind(text_default_4)
                .bind(text_display_4.0)
                .bind::<Option<i32>>(None)
                .bind::<Option<i32>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None)
                .bind::<Option<String>>(None);

            query.execute(&init.db).await.unwrap();
        }
        {
            let query = sqlx::query(
                "INSERT INTO registration_schema_select_options(id, 
                    schema_item,
                    idx,
                    name,
                    product_id
                ) VALUES 
                    (?, ?, ?, ?, ?),
                    (?, ?, ?, ?, ?),
                    (?, ?, ?, ?, ?)",
            );

            // option 1
            let query = query
                .bind(&select_option1_id)
                .bind(&item3_id)
                .bind(0)
                .bind(select_option1_name)
                .bind(select_option1_product_id);

            // option 2
            let query = query
                .bind(&select_option2_id)
                .bind(&item3_id)
                .bind(1)
                .bind(select_option2_name)
                .bind(select_option2_product_id);

            // option 3
            let query = query
                .bind(&select_option3_id)
                .bind(&item3_id)
                .bind(2)
                .bind(select_option3_name)
                .bind(select_option3_product_id);

            query.execute(&init.db).await.unwrap();
        }

        let schemas = vec![
            RegistrationSchema {
                event_id: init.event_1.clone(),
                items: vec![
                    RegistrationSchemaItem {
                        id: item1_id,
                        name: item1_name.to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::Text(TextType {
                                default: text_default.to_owned(),
                                display: text_display.1 as i32,
                            })),
                        }),
                    },
                    RegistrationSchemaItem {
                        id: item2_id,
                        name: item2_name.to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::Checkbox(CheckboxType {
                                default: checkbox_default,
                            })),
                        }),
                    },
                    RegistrationSchemaItem {
                        id: item3_id,
                        name: item3_name.to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::Select(SelectType {
                                default: select_default,
                                display: select_display.1 as i32,
                                options: vec![
                                    SelectOption {
                                        id: select_option1_id,
                                        name: select_option1_name.to_owned(),
                                        product_id: select_option1_product_id.to_owned(),
                                    },
                                    SelectOption {
                                        id: select_option2_id,
                                        name: select_option2_name.to_owned(),
                                        product_id: select_option2_product_id.to_owned(),
                                    },
                                    SelectOption {
                                        id: select_option3_id,
                                        name: select_option3_name.to_owned(),
                                        product_id: select_option3_product_id.to_owned(),
                                    },
                                ],
                            })),
                        }),
                    },
                ],
            },
            RegistrationSchema {
                event_id: init.event_2.clone(),
                items: vec![RegistrationSchemaItem {
                    id: item4_id,
                    name: item4_name.to_owned(),
                    r#type: Some(RegistrationSchemaItemType {
                        r#type: Some(ItemType::Text(TextType {
                            default: text_default_4.to_owned(),
                            display: text_display_4.1 as i32,
                        })),
                    }),
                }],
            },
        ];

        schemas
    }

    #[tokio::test]
    async fn insert() {
        let init = init_db().await;

        let db_ptr = Arc::new(init.db);

        let store = SqliteStore::new(db_ptr.clone());

        let mut schemas = vec![
            RegistrationSchema {
                event_id: init.event_1,
                items: vec![
                    RegistrationSchemaItem {
                        id: "".to_owned(),
                        name: "field 1".to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::Text(TextType {
                                default: "some default".to_owned(),
                                display: text_type::Display::Small as i32,
                            })),
                        }),
                    },
                    RegistrationSchemaItem {
                        id: "".to_owned(),
                        name: "field 2".to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::Select(SelectType {
                                default: 0,
                                display: select_type::Display::Radio as i32,
                                options: vec![SelectOption {
                                    id: "".to_owned(),
                                    name: "option 1".to_owned(),
                                    product_id: "product 1".to_owned(),
                                }],
                            })),
                        }),
                    },
                ],
            },
            RegistrationSchema {
                event_id: init.event_2,
                items: vec![
                    RegistrationSchemaItem {
                        id: "".to_owned(),
                        name: "field 3".to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::Checkbox(CheckboxType { default: true })),
                        }),
                    },
                    RegistrationSchemaItem {
                        id: "".to_owned(),
                        name: "field 4".to_owned(),
                        r#type: Some(RegistrationSchemaItemType {
                            r#type: Some(ItemType::MultiSelect(MultiSelectType {
                                defaults: vec![0, 1],
                                display: multi_select_type::Display::MultiselectBox as i32,
                                options: vec![
                                    SelectOption {
                                        id: "".to_owned(),
                                        name: "option 2".to_owned(),
                                        product_id: "product 2".to_owned(),
                                    },
                                    SelectOption {
                                        id: "".to_owned(),
                                        name: "option 3".to_owned(),
                                        product_id: "product 3".to_owned(),
                                    },
                                    SelectOption {
                                        id: "".to_owned(),
                                        name: "option 4".to_owned(),
                                        product_id: "product 4".to_owned(),
                                    },
                                ],
                            })),
                        }),
                    },
                ],
            },
        ];

        let returned_schemas = store.upsert(schemas.clone()).await.unwrap();

        assert_eq!(schemas.len(), returned_schemas.len());

        let returned_schema_map = returned_schemas
            .into_iter()
            .map(|schema| (schema.event_id.clone(), schema))
            .collect::<HashMap<_, _>>();

        for schema in schemas.iter_mut() {
            let returned_schema = returned_schema_map.get(&schema.event_id).unwrap();

            for (schema_item, returned_item) in
                schema.items.iter_mut().zip(returned_schema.items.iter())
            {
                schema_item.id = returned_item.id.clone();
                let options = match schema_item
                    .r#type
                    .as_mut()
                    .unwrap()
                    .r#type
                    .as_mut()
                    .unwrap()
                {
                    ItemType::Select(schema_select) => {
                        if let ItemType::Select(returned_select) = returned_item
                            .r#type
                            .as_ref()
                            .unwrap()
                            .r#type
                            .as_ref()
                            .unwrap()
                        {
                            Some((&mut schema_select.options, &returned_select.options))
                        } else {
                            panic!()
                        }
                    }
                    ItemType::MultiSelect(schema_select) => {
                        if let ItemType::MultiSelect(returned_select) = returned_item
                            .r#type
                            .as_ref()
                            .unwrap()
                            .r#type
                            .as_ref()
                            .unwrap()
                        {
                            Some((&mut schema_select.options, &returned_select.options))
                        } else {
                            panic!()
                        }
                    }
                    _ => None,
                };

                if let Some((schema_options, returned_options)) = options {
                    for (schema_option, returned_option) in
                        schema_options.iter_mut().zip(returned_options.iter())
                    {
                        schema_option.id = returned_option.id.clone();
                    }
                }
            }

            assert_eq!(schema, returned_schema);
        }

        let store_row: Vec<ItemRow> = sqlx::query_as("SELECT * FROM registration_schema_items")
            .fetch_all(&*db_ptr)
            .await
            .unwrap();

        let store_options_row: Vec<OptionRow> =
            sqlx::query_as("SELECT * FROM registration_schema_select_options")
                .fetch_all(&*db_ptr)
                .await
                .unwrap();

        let store_schemas = items_to_schema(
            store_row.into_iter().map(|row| row.to_item().unwrap()),
            store_options_row
                .into_iter()
                .map(|row| row.to_option().unwrap()),
        );

        assert_eq!(schemas.len(), store_schemas.len());

        let store_schema_map = store_schemas
            .into_iter()
            .map(|schema| (schema.event_id.clone(), schema))
            .collect::<HashMap<_, _>>();

        for schema in schemas.iter() {
            let store_schema = store_schema_map.get(&schema.event_id).unwrap();
            assert_eq!(schema, store_schema);
        }
    }

    #[tokio::test]
    async fn update() {
        let init = init_db().await;

        let mut schemas = test_data(&init).await;

        schemas[0].items.swap(0, 1);

        schemas[0].items[0].name = "item 1 updated".to_owned();
        match schemas[0].items[0]
            .r#type
            .as_mut()
            .unwrap()
            .r#type
            .as_mut()
            .unwrap()
        {
            ItemType::Checkbox(checkbox) => checkbox.default = !checkbox.default,
            _ => panic!("{:?}", schemas[0].items[0]),
        }

        match schemas[0].items[1]
            .r#type
            .as_mut()
            .unwrap()
            .r#type
            .as_mut()
            .unwrap()
        {
            ItemType::Text(text) => text.display = text_type::Display::Small as i32,
            _ => panic!("{:?}", schemas[0].items[1]),
        }

        match schemas[0].items[2]
            .r#type
            .as_mut()
            .unwrap()
            .r#type
            .as_mut()
            .unwrap()
        {
            ItemType::Select(select) => {
                select.default = 0;
                select.options.swap(0, 1);
                select.options[1].product_id = "updated product id".to_owned();
                select.options.remove(2);
            }
            _ => panic!("{:?}", schemas[0].items[2]),
        }

        let db_ptr = Arc::new(init.db);
        let store = SqliteStore::new(db_ptr.clone());

        let returned_schemas = store.upsert(schemas.clone()).await.unwrap();

        assert_eq!(schemas.len(), returned_schemas.len());
        let returned_schema_map = returned_schemas
            .into_iter()
            .map(|schema| (schema.event_id.clone(), schema))
            .collect::<HashMap<_, _>>();

        for schema in schemas.iter_mut() {
            let returned_schema = returned_schema_map.get(&schema.event_id).unwrap();
            assert_eq!(schema, returned_schema);
        }

        let store_row: Vec<ItemRow> = sqlx::query_as("SELECT * FROM registration_schema_items")
            .fetch_all(&*db_ptr)
            .await
            .unwrap();

        let store_options_row: Vec<OptionRow> =
            sqlx::query_as("SELECT * FROM registration_schema_select_options")
                .fetch_all(&*db_ptr)
                .await
                .unwrap();

        let store_schemas = items_to_schema(
            store_row.into_iter().map(|row| row.to_item().unwrap()),
            store_options_row
                .into_iter()
                .map(|row| row.to_option().unwrap()),
        );

        assert_eq!(schemas.len(), store_schemas.len());

        let store_schema_map = store_schemas
            .into_iter()
            .map(|schema| (schema.event_id.clone(), schema))
            .collect::<HashMap<_, _>>();

        for schema in schemas.iter() {
            let store_schema = store_schema_map.get(&schema.event_id).unwrap();
            assert_eq!(schema, store_schema);
        }
    }

    enum UpdateDoesNotExistTests {
        BadEventId,
        BadItemId,
        BadOptionId,
    }
    #[test_case(UpdateDoesNotExistTests::BadEventId ; "bad event id")]
    #[test_case(UpdateDoesNotExistTests::BadItemId ; "bad item id")]
    #[test_case(UpdateDoesNotExistTests::BadOptionId ; "bad option id")]
    #[tokio::test]
    async fn update_does_not_exist(test_name: UpdateDoesNotExistTests) {
        let init = init_db().await;
        let test_data = test_data(&init).await;

        struct TestCase {
            id: String,
            schema: RegistrationSchema,
        }
        let tc = match test_name {
            UpdateDoesNotExistTests::BadEventId => {
                let id = new_id();
                TestCase {
                    id: id.clone(),
                    schema: RegistrationSchema {
                        event_id: id.clone(),
                        items: Vec::new(),
                    },
                }
            }
            UpdateDoesNotExistTests::BadItemId => {
                let id = new_id();
                let mut schema = test_data[0].clone();
                schema.items[0].id = id.clone();
                TestCase { id, schema }
            }
            UpdateDoesNotExistTests::BadOptionId => {
                let id = new_id();
                let mut schema = test_data[0].clone();
                match schema.items[2]
                    .r#type
                    .as_mut()
                    .unwrap()
                    .r#type
                    .as_mut()
                    .unwrap()
                {
                    ItemType::Select(select) => select.options[0].id = id.clone(),
                    _ => panic!(),
                }
                TestCase { id, schema }
            }
        };

        let store = SqliteStore::new(Arc::new(init.db));
        let result = store.upsert(vec![tc.schema]).await;
        match result {
            Ok(_) => panic!("no error returned"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, tc.id),
            _ => panic!("incorrect error type: {:?}", result),
        };
    }

    #[tokio::test]
    async fn list_all() {
        let init = init_db().await;
        let schemas = test_data(&init).await;
        let store = SqliteStore::new(Arc::new(init.db));
        let returned_schemas = store.list(Vec::new()).await.unwrap();
        let returned_schema_map = returned_schemas
            .into_iter()
            .map(|schema| (schema.event_id.clone(), schema))
            .collect::<HashMap<_, _>>();

        for schema in schemas.iter() {
            let returned_schema = returned_schema_map.get(&schema.event_id).unwrap();
            assert_eq!(schema, returned_schema);
        }
    }

    #[tokio::test]
    async fn list_some() {
        let init = init_db().await;
        let schemas = test_data(&init).await;

        let store = SqliteStore::new(Arc::new(init.db));

        let returned_schemas = store.list(vec![init.event_1]).await.unwrap();

        assert_eq!(returned_schemas.len(), 1);
        assert_eq!(schemas[0], returned_schemas[0]);
    }

    #[tokio::test]
    async fn list_does_not_exist() {
        let init = init_db().await;
        let _ = test_data(&init).await;

        let store = SqliteStore::new(Arc::new(init.db));

        let id = new_id();
        let result = store.list(vec![id.clone()]).await;
        match result {
            Ok(_) => panic!("no error returned"),
            Err(Error::IdDoesNotExist(err_id)) => assert_eq!(err_id, id),
            _ => panic!("incorrect error type: {:?}", result),
        };
    }

    #[tokio::test]
    async fn delete_one() {
        let init = init_db().await;
        let _ = test_data(&init).await;

        let db_ptr = Arc::new(init.db);
        let store = SqliteStore::new(db_ptr.clone());

        store.delete(&vec![init.event_1]).await.unwrap();

        let event_ids: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT event FROM registration_schema_items")
                .fetch_all(&*db_ptr)
                .await
                .unwrap();

        assert_eq!(event_ids.len(), 1);
        assert_eq!(event_ids[0].0, init.event_2);

        let (object_count,): (u32,) =
            sqlx::query_as("SELECT COUNT(*) FROM registration_schema_select_options")
                .fetch_one(&*db_ptr)
                .await
                .unwrap();

        assert_eq!(object_count, 0);
    }
}
