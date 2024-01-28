use std::sync::Arc;

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use common::proto::{
    self, compound_user_query, user::Password, user_query, DeleteUsersRequest, DeleteUsersResponse,
    QueryUsersRequest, QueryUsersResponse, UpsertUsersRequest, UpsertUsersResponse, UserQuery,
};
use rand::rngs::OsRng;
use tonic::{Request, Response, Status};

use crate::store::{
    user::{self, PasswordType, Query, Store},
    CompoundOperator, CompoundQuery,
};

use super::{
    common::{to_logical_string_query, validate_string_query},
    store_error_to_status, ValidationError,
};

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn proto_to_user(proto_user: proto::User) -> Result<user::User, Status> {
    let password = match proto_user.password.unwrap() {
        Password::Set(password) => {
            let hashed_password = Argon2::default()
                .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
                .map_err(|e| Status::internal(format!("unable to hash password: {}", e)))?
                .serialize();

            PasswordType::Set(hashed_password)
        }
        Password::Unset(_) => PasswordType::Unset,
        Password::Unchanged(_) => PasswordType::Unchanged,
    };

    Ok(user::User {
        id: proto_user.id,
        email: proto_user.email,
        password,
        display_name: proto_user.display_name,
    })
}

fn user_to_proto(user: user::User) -> proto::User {
    proto::User {
        id: user.id,
        email: user.email,
        password: None,
        display_name: user.display_name,
    }
}

fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password == "" {
        return Err(ValidationError::new_empty("password"));
    }

    Ok(())
}

fn validate_user(user: &proto::User) -> Result<(), ValidationError> {
    if user.email == "" {
        return Err(ValidationError::new_empty("email"));
    }

    if user.display_name == "" {
        return Err(ValidationError::new_empty("display_name"));
    }

    let password = match &user.password {
        Some(password) => password,
        None => return Err(ValidationError::new_empty("password")),
    };

    if user.id == "" && matches!(password, Password::Unchanged(())) {
        return Err(ValidationError::new_empty("password"));
    }

    if let Password::Set(password) = password {
        validate_password(password)?;
    }

    Ok(())
}

fn validate_query(query: &UserQuery) -> Result<(), ValidationError> {
    let query = match &query.query {
        Some(query) => query,
        None => return Err(ValidationError::new_empty("query")),
    };

    match query {
        user_query::Query::Email(email_query) => {
            validate_string_query(&email_query).map_err(|e| e.with_context("query.email_query"))?;
        }
        user_query::Query::Compound(compound_query) => {
            if compound_user_query::Operator::try_from(compound_query.operator).is_err() {
                return Err(ValidationError::new_invalid_enum("query.compound.operator"));
            }

            for (i, query) in compound_query.queries.iter().enumerate() {
                validate_query(query)
                    .map_err(|e| e.with_context(&format!("query.compound.queries[{}]", i)))?;
            }
        }
    }

    Ok(())
}

impl From<UserQuery> for Query {
    fn from(query: UserQuery) -> Self {
        match query.query.unwrap() {
            user_query::Query::Email(email_query) => {
                Query::Email(to_logical_string_query(email_query))
            }

            user_query::Query::Compound(compound_query) => {
                let operator =
                    match compound_user_query::Operator::try_from(compound_query.operator).unwrap()
                    {
                        compound_user_query::Operator::And => CompoundOperator::And,
                        compound_user_query::Operator::Or => CompoundOperator::Or,
                    };

                let queries = compound_query
                    .queries
                    .into_iter()
                    .map(|query| query.into())
                    .collect();

                Query::CompoundQuery(CompoundQuery { operator, queries })
            }
        }
    }
}

#[tonic::async_trait]
impl<StoreType: Store> proto::user_service_server::UserService for Service<StoreType> {
    async fn upsert_users(
        &self,
        request: Request<UpsertUsersRequest>,
    ) -> Result<Response<UpsertUsersResponse>, Status> {
        let request_users = request.into_inner().users;

        for (i, user) in request_users.iter().enumerate() {
            validate_user(user)
                .map_err(|e| -> Status { e.with_context(&format!("users[{}]", i)).into() })?
        }

        let store_users = request_users
            .into_iter()
            .map(proto_to_user)
            .collect::<Result<Vec<_>, _>>()?;

        let users = self
            .store
            .upsert(store_users)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(UpsertUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
        }))
    }

    async fn query_users(
        &self,
        request: Request<QueryUsersRequest>,
    ) -> Result<Response<QueryUsersResponse>, Status> {
        let query = request.into_inner().query;
        let users = match query {
            Some(query) => {
                validate_query(&query).map_err(|e| -> Status { e.with_context("query").into() })?;

                self.store
                    .query(query.into())
                    .await
                    .map_err(|e| store_error_to_status(e))?
            }
            None => self
                .store
                .list()
                .await
                .map_err(|e| store_error_to_status(e))?,
        };

        Ok(Response::new(QueryUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
        }))
    }

    async fn delete_users(
        &self,
        request: Request<DeleteUsersRequest>,
    ) -> Result<Response<DeleteUsersResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteUsersResponse {}))
    }
}
