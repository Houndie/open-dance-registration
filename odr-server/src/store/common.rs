use sqlx::SqlitePool;
use uuid::Uuid;

use super::Error;

pub fn new_id() -> String {
    Uuid::now_v7()
        .hyphenated()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_owned()
}

pub async fn ids_in_table<'a, Iter>(
    pool: &SqlitePool,
    table: &'static str,
    ids: Iter,
) -> Result<(), Error>
where
    Iter: IntoIterator<Item = &'a str> + Clone,
{
    let select_where_clause: String =
        itertools::Itertools::intersperse(ids.clone().into_iter().map(|_| "(?)"), ",").collect();

    let query = format!("WITH valid_ids AS (SELECT column1 FROM ( VALUES {0} )) SELECT column1 FROM valid_ids LEFT JOIN {1} ON {1}.id = valid_ids.column1 WHERE {1}.id IS NULL", select_where_clause, table);

    let mut select_query_builder = sqlx::query_as(&query);

    for id in ids.into_iter() {
        select_query_builder = select_query_builder.bind(id);
    }

    let missing_ids: Vec<(String,)> = select_query_builder
        .fetch_all(pool)
        .await
        .map_err(Error::CheckExistsError)?;

    if !missing_ids.is_empty() {
        return Err(Error::IdDoesNotExist(missing_ids[0].0.clone()));
    };

    Ok(())
}
