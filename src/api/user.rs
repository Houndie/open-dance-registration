use crate::{
    api::{
        authorization_state_to_status, common::try_logical_string_query,
        err_missing_claims_context, middleware::authentication::ClaimsContext,
        store_error_to_status, ValidationError,
    },
    password,
    proto::{
        self, compound_user_query, permission_role, user::Password, user_query, DeleteUsersRequest,
        DeleteUsersResponse, PermissionRole, QueryUsersRequest, QueryUsersResponse,
        UpsertUsersRequest, UpsertUsersResponse, UserQuery,
    },
    store::{
        permission::{self, Store as PermissionStore},
        user::{self, PasswordType, Query, Store as UserStore},
        CompoundOperator, CompoundQuery,
    },
    user::hash_password,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

fn proto_to_user(proto_user: proto::User) -> Result<user::User, Status> {
    let password = match proto_user.password.unwrap() {
        Password::Set(password) => {
            let hashed_password = hash_password(password.as_str())
                .map_err(|e| Status::internal(format!("unable to hash password: {}", e)))?;

            PasswordType::Set(hashed_password)
        }
        Password::Unset(_) => PasswordType::Unset,
        Password::Unchanged(_) => PasswordType::Unchanged,
    };

    Ok(user::User {
        id: proto_user.id,
        username: proto_user.username,
        password,
        email: proto_user.email,
    })
}

fn user_to_proto(user: user::User) -> proto::User {
    proto::User {
        id: user.id,
        username: user.username,
        password: None,
        email: user.email,
    }
}

fn validate_user(user: &proto::User) -> Result<(), ValidationError> {
    if user.email == "" {
        return Err(ValidationError::new_empty("email"));
    }

    if user.username == "" {
        return Err(ValidationError::new_empty("display_name"));
    }

    let password = match &user.password {
        Some(password) => password,
        None => return Err(ValidationError::new_empty("password")),
    };

    match password {
        Password::Set(password) => {
            if !password::Validation::new(password).is_valid() {
                return Err(ValidationError::new_invalid_value("password"));
            }
        }
        Password::Unset(_) => (),
        Password::Unchanged(_) => {
            if user.id == "" {
                return Err(ValidationError::new_empty("password"));
            }
        }
    };

    Ok(())
}

fn try_parse_user_query(query: UserQuery) -> Result<Query, ValidationError> {
    match query.query {
        Some(user_query::Query::Email(email_query)) => Ok(Query::Email(
            try_logical_string_query(email_query).map_err(|e| e.with_context("query.email"))?,
        )),

        Some(user_query::Query::Id(id_query)) => Ok(Query::Id(
            try_logical_string_query(id_query).map_err(|e| e.with_context("query.id"))?,
        )),

        Some(user_query::Query::Username(display_name_query)) => Ok(Query::Username(
            try_logical_string_query(display_name_query).map_err(|e| e.with_context("query.id"))?,
        )),

        Some(user_query::Query::Compound(compound_query)) => {
            let operator = match compound_user_query::Operator::try_from(compound_query.operator) {
                Ok(compound_user_query::Operator::And) => CompoundOperator::And,
                Ok(compound_user_query::Operator::Or) => CompoundOperator::Or,
                Err(_) => return Err(ValidationError::new_invalid_enum("query.compound.operator")),
            };

            let queries = compound_query
                .queries
                .into_iter()
                .enumerate()
                .map(|(idx, query)| {
                    try_parse_user_query(query).map_err(|e: ValidationError| {
                        e.with_context(&format!("query.compound.queries[{}]", idx))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Query::CompoundQuery(CompoundQuery { operator, queries }))
        }
        None => Err(ValidationError::new_empty("query")),
    }
}

pub struct Service<UStore: UserStore, PStore: PermissionStore> {
    user_store: Arc<UStore>,
    permission_store: Arc<PStore>,
}

impl<UStore: UserStore, PStore: PermissionStore> Service<UStore, PStore> {
    pub fn new(user_store: Arc<UStore>, permission_store: Arc<PStore>) -> Self {
        Service {
            user_store,
            permission_store,
        }
    }
}

fn required_permissions(user_id: &str) -> Vec<proto::Permission> {
    vec![proto::Permission {
        id: "".to_string(),
        user_id: user_id.to_string(),
        role: Some(PermissionRole {
            role: Some(permission_role::Role::ServerAdmin(())),
        }),
    }]
}

async fn strip_users<PStore: PermissionStore>(
    permission_store: &PStore,
    users: Vec<proto::User>,
    this_user_id: &str,
) -> Result<Vec<proto::User>, Status> {
    if !users.iter().any(|user| user.id == this_user_id) {
        return Ok(users);
    }

    let required_permissions = required_permissions(this_user_id);

    let failed_permissions = permission_store
        .permission_check(required_permissions)
        .await
        .map_err(store_error_to_status)?;

    if failed_permissions.is_empty() {
        return Ok(users);
    }

    let stripped_users = users
        .into_iter()
        .map(|user| {
            if user.id == this_user_id {
                return user;
            };

            let mut stripped_user = proto::User::default();
            stripped_user.id = user.id;
            stripped_user.username = user.username;

            stripped_user
        })
        .collect();

    return Ok(stripped_users);
}

#[tonic::async_trait]
impl<UStore: UserStore, PStore: PermissionStore> proto::user_service_server::UserService
    for Service<UStore, PStore>
{
    async fn upsert_users(
        &self,
        request: Request<UpsertUsersRequest>,
    ) -> Result<Response<UpsertUsersResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let request_users = request.users;

        for (i, user) in request_users.iter().enumerate() {
            validate_user(user)
                .map_err(|e| -> Status { e.with_context(&format!("users[{}]", i)).into() })?
        }

        if request_users
            .iter()
            .any(|user| user.id != claims_context.claims.sub)
        {
            let required_permissions = required_permissions(&claims_context.claims.sub);

            let failed_permissions = self
                .permission_store
                .permission_check(required_permissions)
                .await
                .map_err(store_error_to_status)?;

            authorization_state_to_status(failed_permissions)?;
        }

        let store_users = request_users
            .into_iter()
            .map(proto_to_user)
            .collect::<Result<Vec<_>, _>>()?;

        let users = self
            .user_store
            .upsert(store_users)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(UpsertUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
        }))
    }

    async fn query_users(
        &self,
        request: Request<QueryUsersRequest>,
    ) -> Result<Response<QueryUsersResponse>, Status> {
        let (_, extensions, request) = request.into_parts();

        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let query = request.query;
        let query = query.map(|query| try_parse_user_query(query)).transpose()?;

        let users = self
            .user_store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        let proto_users = users.into_iter().map(user_to_proto).collect::<Vec<_>>();

        let stripped_users = strip_users(
            &*self.permission_store,
            proto_users,
            &claims_context.claims.sub,
        )
        .await?;

        Ok(Response::new(QueryUsersResponse {
            users: stripped_users,
        }))
    }

    async fn delete_users(
        &self,
        request: Request<DeleteUsersRequest>,
    ) -> Result<Response<DeleteUsersResponse>, Status> {
        let (_, extensions, request) = request.into_parts();
        let claims_context = extensions
            .get::<ClaimsContext>()
            .ok_or_else(err_missing_claims_context)?;

        let ids = request.ids;

        let to_be_deleted = self
            .permission_store
            .query(Some(&permission::Query::CompoundQuery(CompoundQuery {
                operator: CompoundOperator::Or,
                queries: ids
                    .iter()
                    .map(|id| permission::Query::Id(permission::IdQuery::Equals(id.clone())))
                    .collect(),
            })))
            .await
            .map_err(store_error_to_status)?;

        if to_be_deleted
            .iter()
            .any(|user| user.id != claims_context.claims.sub)
        {
            let required_permissions = required_permissions(&claims_context.claims.sub);

            let failed_permissions = self
                .permission_store
                .permission_check(required_permissions)
                .await
                .map_err(store_error_to_status)?;

            authorization_state_to_status(failed_permissions)?;
        }

        self.user_store
            .delete(&ids)
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteUsersResponse {}))
    }
}
