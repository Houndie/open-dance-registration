use crate::{
    proto::{permission_role, EventRole, OrganizationRole, Permission, PermissionRole},
    store::{
        common::{ids_in_table, new_id},
        Bindable, Error, Queryable,
    },
};
use mockall::automock;
use sqlx::SqlitePool;
use std::{future::Future, sync::Arc};

#[derive(sqlx::FromRow)]
struct PermissionRow {
    id: String,
    user: String,
    role: String,
    organization: Option<String>,
    event: Option<String>,
}

impl TryFrom<PermissionRow> for Permission {
    type Error = Error;

    fn try_from(row: PermissionRow) -> Result<Self, Error> {
        let role = match row.role.as_str() {
            "SERVER_ADMIN" => permission_role::Role::ServerAdmin(()),
            "ORGANIZATION_ADMIN" => permission_role::Role::OrganizationAdmin(OrganizationRole {
                organization_id: row
                    .organization
                    .ok_or(Error::ColumnParseError("organization"))?,
            }),
            "ORGANIZATION_VIEWER" => permission_role::Role::OrganizationViewer(OrganizationRole {
                organization_id: row
                    .organization
                    .ok_or(Error::ColumnParseError("organization"))?,
            }),
            "EVENT_ADMIN" => permission_role::Role::EventAdmin(EventRole {
                event_id: row.event.ok_or(Error::ColumnParseError("event"))?,
            }),
            "EVENT_EDITOR" => permission_role::Role::EventEditor(EventRole {
                event_id: row.event.ok_or(Error::ColumnParseError("event"))?,
            }),
            "EVENT_VIEWER" => permission_role::Role::EventViewer(EventRole {
                event_id: row.event.ok_or(Error::ColumnParseError("event"))?,
            }),
            _ => return Err(Error::ColumnParseError("role")),
        };

        Ok(Permission {
            id: row.id,
            user_id: row.user,
            role: Some(PermissionRole { role: Some(role) }),
        })
    }
}

pub struct IdField;

impl super::Field for IdField {
    type Item = String;

    fn field() -> &'static str {
        "p.id"
    }
}

pub type IdQuery = super::LogicalQuery<IdField>;

pub struct UserIdField;

impl super::Field for UserIdField {
    type Item = String;

    fn field() -> &'static str {
        "p.user"
    }
}

pub type UserIdQuery = super::LogicalQuery<UserIdField>;

pub enum PermissionRoleQuery {
    Is(PermissionRole),
    IsNot(PermissionRole),
}

impl Queryable for PermissionRoleQuery {
    fn where_clause(&self) -> String {
        match self {
            PermissionRoleQuery::Is(role) => match role.role.as_ref().unwrap() {
                permission_role::Role::ServerAdmin(_) => "p.role = 'SERVER_ADMIN'".to_string(),
                permission_role::Role::OrganizationAdmin(_) => {
                    "p.role = 'ORGANIZATION_ADMIN' AND p.organization = ?".to_string()
                }
                permission_role::Role::OrganizationViewer(_) => {
                    "p.role = 'ORGANIZATION_VIEWER' AND p.organization = ?".to_string()
                }
                permission_role::Role::EventAdmin(_) => {
                    "p.role = 'EVENT_ADMIN' AND p.event = ?".to_string()
                }
                permission_role::Role::EventEditor(_) => {
                    "p.role = 'EVENT_EDITOR' AND p.event = ?".to_string()
                }
                permission_role::Role::EventViewer(_) => {
                    "p.role = 'EVENT_VIEWER' AND p.event = ?".to_string()
                }
            },
            PermissionRoleQuery::IsNot(role) => match role.role.as_ref().unwrap() {
                permission_role::Role::ServerAdmin(_) => "p.role != 'SERVER_ADMIN'".to_string(),
                permission_role::Role::OrganizationAdmin(_) => {
                    "p.role != 'ORGANIZATION_ADMIN' OR p.organization != ?".to_string()
                }
                permission_role::Role::OrganizationViewer(_) => {
                    "p.role != 'ORGANIZATION_VIEWER' OR p.organization != ?".to_string()
                }
                permission_role::Role::EventAdmin(_) => {
                    "p.role != 'EVENT_ADMIN' OR p.event != ?".to_string()
                }
                permission_role::Role::EventEditor(_) => {
                    "p.role != 'EVENT_EDITOR' OR p.event = ?".to_string()
                }
                permission_role::Role::EventViewer(_) => {
                    "p.role != 'EVENT_VIEWER' OR p.event = ?".to_string()
                }
            },
        }
    }
}

impl<'q, DB: sqlx::Database> Bindable<'q, DB> for PermissionRoleQuery
where
    String: sqlx::Encode<'q, DB> + sqlx::Type<DB> + Sync,
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
            PermissionRoleQuery::Is(role) => match role.role.as_ref().unwrap() {
                permission_role::Role::ServerAdmin(_) => query_builder,
                permission_role::Role::OrganizationAdmin(r) => {
                    query_builder.bind(&r.organization_id)
                }
                permission_role::Role::OrganizationViewer(r) => {
                    query_builder.bind(&r.organization_id)
                }
                permission_role::Role::EventAdmin(r) => query_builder.bind(&r.event_id),
                permission_role::Role::EventEditor(r) => query_builder.bind(&r.event_id),
                permission_role::Role::EventViewer(r) => query_builder.bind(&r.event_id),
            },
            PermissionRoleQuery::IsNot(role) => match role.role.as_ref().unwrap() {
                permission_role::Role::ServerAdmin(_) => query_builder,
                permission_role::Role::OrganizationAdmin(r) => {
                    query_builder.bind(&r.organization_id)
                }
                permission_role::Role::OrganizationViewer(r) => {
                    query_builder.bind(&r.organization_id)
                }
                permission_role::Role::EventAdmin(r) => query_builder.bind(&r.event_id),
                permission_role::Role::EventEditor(r) => query_builder.bind(&r.event_id),
                permission_role::Role::EventViewer(r) => query_builder.bind(&r.event_id),
            },
        }
    }
}

pub enum Query {
    Id(IdQuery),
    UserId(UserIdQuery),
    Role(PermissionRoleQuery),
    CompoundQuery(super::CompoundQuery<Query>),
}

impl Queryable for Query {
    fn where_clause(&self) -> String {
        match self {
            Query::Id(q) => q.where_clause(),
            Query::UserId(q) => q.where_clause(),
            Query::Role(q) => q.where_clause(),
            Query::CompoundQuery(compound_query) => compound_query.where_clause(),
        }
    }
}

impl<'q, DB: sqlx::Database> super::Bindable<'q, DB> for Query
where
    <IdField as super::Field>::Item: sqlx::Type<DB> + sqlx::Encode<'q, DB>,
    <UserIdField as super::Field>::Item: sqlx::Type<DB> + sqlx::Encode<'q, DB>,
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
            Query::UserId(q) => q.bind(query_builder),
            Query::Role(q) => q.bind(query_builder),
            Query::CompoundQuery(compound_query) => compound_query.bind(query_builder),
        }
    }
}

type QueryBuilder<'q> = sqlx::query::Query<
    'q,
    sqlx::Sqlite,
    <sqlx::Sqlite as sqlx::database::HasArguments<'q>>::Arguments,
>;

fn bind_permission<'q>(
    query_builder: QueryBuilder<'q>,
    permission: &'q Permission,
) -> QueryBuilder<'q> {
    let query_builder = query_builder.bind(&permission.id).bind(&permission.user_id);

    match permission.role.as_ref().unwrap().role.as_ref().unwrap() {
        permission_role::Role::ServerAdmin(_) => query_builder
            .bind("SERVER_ADMIN")
            .bind(None as Option<&str>)
            .bind(None as Option<&str>),
        permission_role::Role::OrganizationAdmin(r) => query_builder
            .bind("ORGANIZATION_ADMIN")
            .bind(Some(r.organization_id.as_str()))
            .bind(None as Option<&str>),
        permission_role::Role::OrganizationViewer(r) => query_builder
            .bind("ORGANIZATION_VIEWER")
            .bind(Some(r.organization_id.as_str()))
            .bind(None as Option<&str>),
        permission_role::Role::EventAdmin(r) => query_builder
            .bind("EVENT_ADMIN")
            .bind(None as Option<&str>)
            .bind(Some(r.event_id.as_str())),
        permission_role::Role::EventEditor(r) => query_builder
            .bind("EVENT_EDITOR")
            .bind(None as Option<&str>)
            .bind(Some(r.event_id.as_str())),
        permission_role::Role::EventViewer(r) => query_builder
            .bind("EVENT_VIEWER")
            .bind(None as Option<&str>)
            .bind(Some(r.event_id.as_str())),
    }
}

#[automock]
pub trait Store: Send + Sync + 'static {
    fn upsert(
        &self,
        users: Vec<Permission>,
    ) -> impl Future<Output = Result<Vec<Permission>, Error>> + Send;
    fn query<'a>(
        &self,
        query: Option<&'a Query>,
    ) -> impl Future<Output = Result<Vec<Permission>, Error>> + Send;
    fn delete(&self, ids: &[String]) -> impl Future<Output = Result<(), Error>> + Send;
    fn permission_check(
        &self,
        requested: Vec<Permission>,
    ) -> impl Future<Output = Result<Vec<Permission>, Error>> + Send;
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
    async fn upsert(&self, permissions: Vec<Permission>) -> Result<Vec<Permission>, Error> {
        if permissions.is_empty() {
            return Ok(Vec::new());
        }

        let organization_ids = permissions
            .iter()
            .filter_map(|p| match p.role.as_ref().unwrap().role.as_ref().unwrap() {
                permission_role::Role::OrganizationAdmin(r) => Some(r.organization_id.as_str()),
                permission_role::Role::OrganizationViewer(r) => Some(r.organization_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        if !organization_ids.is_empty() {
            ids_in_table(&self.pool, "organizations", organization_ids).await?;
        }

        let event_ids = permissions
            .iter()
            .filter_map(|p| match p.role.as_ref().unwrap().role.as_ref().unwrap() {
                permission_role::Role::EventAdmin(r) => Some(r.event_id.as_str()),
                permission_role::Role::EventEditor(r) => Some(r.event_id.as_str()),
                permission_role::Role::EventViewer(r) => Some(r.event_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        if !event_ids.is_empty() {
            ids_in_table(&self.pool, "events", event_ids).await?;
        }

        let user_ids = permissions.iter().map(|p| p.user_id.as_str());
        ids_in_table(&self.pool, "users", user_ids).await?;

        let (inserts, updates): (Vec<_>, Vec<_>) = permissions
            .into_iter()
            .enumerate()
            .partition(|(_, p)| p.id.is_empty());

        if !updates.is_empty() {
            ids_in_table(
                &self.pool,
                "permissions",
                updates.iter().map(|(_, p)| p.id.as_str()),
            )
            .await?;
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(Error::TransactionStartError)?;

        let inserts = inserts
            .into_iter()
            .map(|(idx, mut p)| {
                p.id = new_id();
                (idx, p)
            })
            .collect::<Vec<_>>();

        if !inserts.is_empty() {
            let values_clause: String = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?, ?, ?, ?)").take(inserts.len()),
                ", ",
            )
            .collect();

            let query = format!(
                "INSERT INTO permissions (id, user, role, organization, event) VALUES {}",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = inserts.iter().fold(query_builder, |query_builder, (_, p)| {
                bind_permission(query_builder, p)
            });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(Error::InsertionError)?;
        };

        if !updates.is_empty() {
            let values_clause: String = itertools::Itertools::intersperse(
                std::iter::repeat("(?, ?, ?, ?, ?)").take(updates.len()),
                ", ",
            )
            .collect();

            let query = format!(
                "WITH mydata(id, user, role, organization, event) AS (VALUES {}) 
                    UPDATE permissions
                    SET user = mydata.user, 
                        role = mydata.role,
                        organization = mydata.organization,
                        event = mydata.event
                    FROM mydata
                    WHERE permissions.id = mydata.id",
                values_clause
            );

            let query_builder = sqlx::query(&query);
            let query_builder = updates
                .iter()
                .fold(query_builder, |query_builder, (_, user)| {
                    bind_permission(query_builder, user)
                });

            query_builder
                .execute(&mut *tx)
                .await
                .map_err(Error::UpdateError)?;
        }

        tx.commit().await.map_err(Error::TransactionFailed)?;

        let mut outputs = Vec::new();
        outputs.resize(inserts.len() + updates.len(), Permission::default());

        inserts.into_iter().chain(updates).for_each(|(idx, p)| {
            outputs[idx] = p;
        });

        Ok(outputs)
    }

    async fn query(&self, query: Option<&Query>) -> Result<Vec<Permission>, Error> {
        let base_query_string =
            "SELECT p.id, p.user, p.role, p.organization, p.event FROM permissions p";

        let query_string = match query {
            Some(q) => {
                format!("{} WHERE {};", base_query_string, q.where_clause())
            }
            None => base_query_string.to_string(),
        };

        let query_builder = sqlx::query_as(&query_string);
        let query_builder = match query {
            Some(q) => q.bind(query_builder),
            None => query_builder,
        };

        let rows: Vec<PermissionRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(Error::FetchError)?;

        let permissions = rows
            .into_iter()
            .map(Permission::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(permissions)
    }

    async fn delete(&self, ids: &[String]) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }

        ids_in_table(&self.pool, "permissions", ids.iter().map(|id| id.as_str())).await?;

        let where_clause =
            itertools::Itertools::intersperse(std::iter::repeat("id = ?").take(ids.len()), " OR ")
                .collect::<String>();

        let query = format!("DELETE FROM permissions WHERE {}", where_clause);

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

    async fn permission_check(&self, requested: Vec<Permission>) -> Result<Vec<Permission>, Error> {
        if requested.is_empty() {
            return Ok(Vec::new());
        }

        let values = itertools::Itertools::intersperse(
            std::iter::repeat("('', ?, ?, ?, ?)").take(requested.len()),
            ", ",
        )
        .collect::<String>();

        let query = format!(
            r#"WITH requested(id, user, role, organization, event) as (VALUES {})
            SELECT id, user, role, organization, event FROM requested WHERE NOT EXISTS (
                SELECT 1 FROM permissions p
                LEFT JOIN events e ON p.organization = e.organization
                WHERE user = requested.user AND (
                    (requested.role = 'SERVER_ADMIN' AND p.role = 'SERVER_ADMIN')
                    OR (requested.role = 'ORGANIZATION_ADMIN' AND (
                        p.role = 'SERVER_ADMIN'
                        OR (p.role = 'ORGANIZATION_ADMIN' AND p.organization = requested.organization)
                    ))
                    OR (requested.role = 'ORGANIZATION_VIEWER' AND (
                        p.role = 'SERVER_ADMIN'
                        OR ((p.role = 'ORGANIZATION_ADMIN' OR p.role = 'ORGANIZATION_VIEWER') AND p.organization = requested.organization)
                    ))
                    OR (requested.role = 'EVENT_ADMIN' AND (
                        p.role = 'SERVER_ADMIN'
                        OR (p.role = 'EVENT_ADMIN' AND p.event = requested.event)
                        OR (p.role = 'ORGANIZATION_ADMIN' AND e.id = requested.event)
                    ))
                    OR (requested.role = 'EVENT_EDITOR' AND (
                        p.role = 'SERVER_ADMIN'
                        OR ((p.role = 'EVENT_ADMIN' OR p.role = 'EVENT_EDITOR') AND p.event = requested.event)
                        OR (p.role = 'ORGANIZATION_ADMIN' AND e.id = requested.event)
                    ))
                    OR (requested.role = 'EVENT_VIEWER' AND (
                        p.role = 'SERVER_ADMIN'
                        OR ((p.role = 'EVENT_VIEWER' OR p.role = 'EVENT_EDITOR' OR p.role = 'EVENT_ADMIN') AND p.event = requested.event)
                        OR ((p.role = 'ORGANIZATION_ADMIN' OR p.role = 'ORGANIZATION_VIEWER') AND e.id = requested.event)
                    ))
                )
            )"#,
            values
        );

        let query_builder = sqlx::query_as(&query);
        let query_builder = requested.iter().fold(query_builder, |query_builder, p| {
            let query_builder = query_builder.bind(&p.user_id);

            match p.role.as_ref().unwrap().role.as_ref().unwrap() {
                permission_role::Role::ServerAdmin(_) => query_builder
                    .bind("SERVER_ADMIN")
                    .bind(None as Option<&str>)
                    .bind(None as Option<&str>),
                permission_role::Role::OrganizationAdmin(r) => query_builder
                    .bind("ORGANIZATION_ADMIN")
                    .bind(Some(r.organization_id.as_str()))
                    .bind(None as Option<&str>),
                permission_role::Role::OrganizationViewer(r) => query_builder
                    .bind("ORGANIZATION_VIEWER")
                    .bind(Some(r.organization_id.as_str()))
                    .bind(None as Option<&str>),
                permission_role::Role::EventAdmin(r) => query_builder
                    .bind("EVENT_ADMIN")
                    .bind(None as Option<&str>)
                    .bind(Some(r.event_id.as_str())),
                permission_role::Role::EventEditor(r) => query_builder
                    .bind("EVENT_EDITOR")
                    .bind(None as Option<&str>)
                    .bind(Some(r.event_id.as_str())),
                permission_role::Role::EventViewer(r) => query_builder
                    .bind("EVENT_VIEWER")
                    .bind(None as Option<&str>)
                    .bind(Some(r.event_id.as_str())),
            }
        });

        let rows: Vec<PermissionRow> = query_builder
            .fetch_all(&*self.pool)
            .await
            .map_err(Error::FetchError)?;

        let permissions = rows
            .into_iter()
            .map(Permission::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(permissions)
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionRoleQuery, PermissionRow, Query, SqliteStore, Store};
    use crate::{
        proto::{permission_role, EventRole, OrganizationRole, Permission, PermissionRole},
        store::{common::new_id, Error, LogicalQuery},
    };
    use sqlx::{
        migrate::MigrateDatabase, sqlite::SqliteConnectOptions, ConnectOptions, Sqlite, SqlitePool,
    };
    use std::{str::FromStr, sync::Arc};
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
        sqlx::migrate!("./migrations").run(&db).await.unwrap();

        Init { db }
    }

    async fn test_data(init: &Init) -> Vec<Permission> {
        let user1_id = new_id();
        let user2_id = new_id();
        let org_id = new_id();
        let event1_id = new_id();
        let event2_id = new_id();
        let server_admin = Permission {
            id: new_id(),
            user_id: user1_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::ServerAdmin(())),
            }),
        };
        let organization_admin = Permission {
            id: new_id(),
            user_id: user1_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                    organization_id: org_id.clone(),
                })),
            }),
        };
        let organization_viewer = Permission {
            id: new_id(),
            user_id: user2_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::OrganizationViewer(
                    OrganizationRole {
                        organization_id: org_id.clone(),
                    },
                )),
            }),
        };
        let event_admin = Permission {
            id: new_id(),
            user_id: user2_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventAdmin(EventRole {
                    event_id: event1_id.clone(),
                })),
            }),
        };
        let event_editor = Permission {
            id: new_id(),
            user_id: user2_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventEditor(EventRole {
                    event_id: event2_id.clone(),
                })),
            }),
        };
        let event_viewer = Permission {
            id: new_id(),
            user_id: user2_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventViewer(EventRole {
                    event_id: event2_id.clone(),
                })),
            }),
        };

        sqlx::query(
            "INSERT INTO users (id, email, password, username) VALUES (?, ?, ?, ?), (?, ?, ?, ?)",
        )
        .bind(&user1_id)
        .bind("user1@example.com")
        .bind("password")
        .bind("User 1")
        .bind(&user2_id)
        .bind("user2@example.com")
        .bind("password")
        .bind("User 2")
        .execute(&init.db)
        .await
        .unwrap();

        sqlx::query("INSERT INTO organizations (id, name) VALUES (?, ?)")
            .bind(&org_id)
            .bind("organization")
            .execute(&init.db)
            .await
            .unwrap();

        sqlx::query("INSERT INTO events (id, organization, name) VALUES (?, ?, ?), (?, ?, ?)")
            .bind(&event1_id)
            .bind(&org_id)
            .bind("event1")
            .bind(&event2_id)
            .bind(&org_id)
            .bind("event2")
            .execute(&init.db)
            .await
            .unwrap();

        let query = "INSERT INTO permissions (id, user, role, organization, event) VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?)";

        sqlx::query(query)
            .bind(&server_admin.id)
            .bind(&user1_id)
            .bind("SERVER_ADMIN")
            .bind(None as Option<&str>)
            .bind(None as Option<&str>)
            .bind(&organization_admin.id)
            .bind(&user1_id)
            .bind("ORGANIZATION_ADMIN")
            .bind(Some(&org_id))
            .bind(None as Option<&str>)
            .bind(&organization_viewer.id)
            .bind(&user2_id)
            .bind("ORGANIZATION_VIEWER")
            .bind(Some(&org_id))
            .bind(None as Option<&str>)
            .bind(&event_admin.id)
            .bind(&user2_id)
            .bind("EVENT_ADMIN")
            .bind(None as Option<&str>)
            .bind(Some(&event1_id))
            .bind(&event_editor.id)
            .bind(&user2_id)
            .bind("EVENT_EDITOR")
            .bind(None as Option<&str>)
            .bind(Some(&event2_id))
            .bind(&event_viewer.id)
            .bind(&user2_id)
            .bind("EVENT_VIEWER")
            .bind(None as Option<&str>)
            .bind(Some(&event2_id))
            .execute(&init.db)
            .await
            .unwrap();

        vec![
            server_admin,
            organization_admin,
            organization_viewer,
            event_admin,
            event_editor,
            event_viewer,
        ]
    }

    #[tokio::test]
    async fn insert() {
        let init = init().await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let user_id = new_id();
        let organization_id = new_id();

        let permissions = vec![
            Permission {
                id: "".to_string(),
                user_id: user_id.clone(),
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::ServerAdmin(())),
                }),
            },
            Permission {
                id: "".to_string(),
                user_id: user_id.clone(),
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                        organization_id: organization_id.clone(),
                    })),
                }),
            },
        ];

        sqlx::query("INSERT INTO users (id, email, password, username) VALUES (?, ?, ?, ?)")
            .bind(&user_id)
            .bind("user@example.com")
            .bind("password")
            .bind("User")
            .execute(&*db)
            .await
            .unwrap();

        sqlx::query("INSERT INTO organizations (id, name) VALUES (?, ?)")
            .bind(&organization_id)
            .bind("organization")
            .execute(&*db)
            .await
            .unwrap();

        let returned_permisions = store.upsert(permissions.clone()).await.unwrap();

        let mut permissions = permissions
            .into_iter()
            .zip(returned_permisions.iter())
            .map(|(mut permission, store_permission)| {
                permission.id = store_permission.id.clone();
                permission
            })
            .collect::<Vec<_>>();

        assert_eq!(permissions, returned_permisions);

        let permission_rows: Vec<PermissionRow> =
            sqlx::query_as("SELECT id, user, role, organization, event FROM permissions")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_permissions = permission_rows
            .into_iter()
            .map(Permission::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        permissions.sort_by(|a, b| a.id.cmp(&b.id));
        store_permissions.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(permissions, store_permissions);
    }

    #[tokio::test]
    async fn update() {
        let init = init().await;
        let mut permissions = test_data(&init).await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let organization_id = new_id();
        sqlx::query("INSERT INTO organizations (id, name) VALUES (?, ?)")
            .bind(&organization_id)
            .bind("organization")
            .execute(&*db)
            .await
            .unwrap();

        let user_id = new_id();
        sqlx::query("INSERT INTO users (id, email, password, username) VALUES (?, ?, ?, ?)")
            .bind(&user_id)
            .bind("user@example.com")
            .bind("password")
            .bind("User")
            .execute(&*db)
            .await
            .unwrap();

        permissions[0].user_id = user_id;
        permissions[1].role.as_mut().unwrap().role = Some(
            permission_role::Role::OrganizationViewer(OrganizationRole { organization_id }),
        );

        let returned_permissions = store.upsert(permissions.clone()).await.unwrap();

        assert_eq!(permissions, returned_permissions);

        let permission_rows: Vec<PermissionRow> =
            sqlx::query_as("SELECT id, user, role, organization, event FROM permissions")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_permissions = permission_rows
            .into_iter()
            .map(Permission::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        permissions.sort_by(|a, b| a.id.cmp(&b.id));
        store_permissions.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(permissions, store_permissions);
    }

    #[tokio::test]
    async fn update_does_not_exist() {
        let init = init().await;
        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let id = new_id();

        let user_id = new_id();
        sqlx::query("INSERT INTO users (id, email, password, username) VALUES (?, ?, ?, ?)")
            .bind(&user_id)
            .bind("user@example.com")
            .bind("password")
            .bind("User")
            .execute(&*db)
            .await
            .unwrap();

        let result = store
            .upsert(vec![Permission {
                id: id.clone(),
                user_id,
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::ServerAdmin(())),
                }),
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
        UserId,
        RoleIs,
        NoResults,
    }

    #[test_case(QueryTest::All ; "all")]
    #[test_case(QueryTest::Id ; "id")]
    #[test_case(QueryTest::UserId ; "user id")]
    #[test_case(QueryTest::RoleIs ; "role is")]
    #[test_case(QueryTest::NoResults ; "no results")]
    #[tokio::test]
    async fn query(test_name: QueryTest) {
        let init = init().await;
        let mut permissions = test_data(&init).await;

        struct TestCase {
            query: Option<Query>,
            expected: Vec<Permission>,
        }
        let tc = match test_name {
            QueryTest::All => TestCase {
                query: None,
                expected: permissions,
            },
            QueryTest::Id => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(permissions[0].id.clone()))),
                expected: vec![permissions.remove(0)],
            },
            QueryTest::UserId => TestCase {
                query: Some(Query::UserId(LogicalQuery::Equals(
                    permissions[0].user_id.clone(),
                ))),
                expected: vec![permissions.remove(0), permissions.remove(0)],
            },
            QueryTest::RoleIs => {
                let organization_id =
                    match permissions[1].role.as_ref().unwrap().role.as_ref().unwrap() {
                        permission_role::Role::OrganizationAdmin(r) => r.organization_id.clone(),
                        _ => panic!("unexpected role"),
                    };
                TestCase {
                    query: Some(Query::Role(PermissionRoleQuery::Is(PermissionRole {
                        role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                            organization_id,
                        })),
                    }))),
                    expected: vec![permissions.remove(1)],
                }
            }
            QueryTest::NoResults => TestCase {
                query: Some(Query::Id(LogicalQuery::Equals(new_id()))),
                expected: vec![],
            },
        };

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let mut returned_permissions = store.query(tc.query.as_ref()).await.unwrap();
        returned_permissions.sort_by(|a, b| a.id.cmp(&b.id));

        let mut expected = tc.expected;
        expected.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(expected, returned_permissions);
    }

    #[tokio::test]
    async fn delete() {
        let init = init().await;
        let permissions = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        store.delete(&[permissions[0].id.clone()]).await.unwrap();

        let store_permission_rows: Vec<PermissionRow> =
            sqlx::query_as("SELECT id, user, role, organization, event FROM permissions")
                .fetch_all(&*db)
                .await
                .unwrap();

        let mut store_permissions: Vec<Permission> = store_permission_rows
            .into_iter()
            .map(|row| row.try_into().unwrap())
            .collect::<Vec<_>>();

        store_permissions.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(permissions[1], store_permissions[0]);
    }

    #[tokio::test]
    async fn permission_check() {
        let init = init().await;
        let permissions = test_data(&init).await;

        let db = Arc::new(init.db);
        let store = SqliteStore::new(db.clone());

        let mut event2_viewer_permission = permissions[5].clone();
        event2_viewer_permission.user_id = permissions[0].user_id.clone();

        let failed_permissions = store
            .permission_check(vec![event2_viewer_permission])
            .await
            .unwrap();

        // Allowed, as this user is a server admin
        assert!(failed_permissions.is_empty());

        let event1_viewer_permission = Permission {
            id: "".to_string(),
            user_id: permissions[5].user_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventViewer(EventRole {
                    event_id: match permissions[3].role.as_ref().unwrap().role.as_ref().unwrap() {
                        permission_role::Role::EventAdmin(r) => r.event_id.clone(),
                        _ => panic!("unexpected role"),
                    },
                })),
            }),
        };

        let failed_permissions = store
            .permission_check(vec![event1_viewer_permission])
            .await
            .unwrap();

        // Allowed, as this user is an organization viewer
        assert!(failed_permissions.is_empty());

        let event2_admin_permission = Permission {
            id: "".to_string(),
            user_id: permissions[5].user_id.clone(),
            role: Some(PermissionRole {
                role: Some(permission_role::Role::EventAdmin(EventRole {
                    event_id: match permissions[4].role.as_ref().unwrap().role.as_ref().unwrap() {
                        permission_role::Role::EventEditor(r) => r.event_id.clone(),
                        _ => panic!("unexpected role"),
                    },
                })),
            }),
        };

        let failed_permissions = store
            .permission_check(vec![event2_admin_permission])
            .await
            .unwrap();

        // Denied, as this user is not any kind of admin
        assert!(!failed_permissions.is_empty());
    }
}
