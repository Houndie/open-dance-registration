use crate::{
    api::{common::try_logical_string_query, store_error_to_status, ValidationError},
    password,
    proto::{
        self, compound_user_query, user::Password, user_query, DeleteUsersRequest,
        DeleteUsersResponse, QueryUsersRequest, QueryUsersResponse, UpsertUsersRequest,
        UpsertUsersResponse, UserQuery,
    },
    store::{
        user::{self, PasswordType, Query, Store},
        CompoundOperator, CompoundQuery,
    },
    user::hash_password,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

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
            let hashed_password = hash_password(password.as_str())
                .map_err(|e| Status::internal(format!("unable to hash password: {}", e)))?;

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

        Some(user_query::Query::DisplayName(display_name_query)) => Ok(Query::DisplayName(
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
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(UpsertUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
        }))
    }

    async fn query_users(
        &self,
        request: Request<QueryUsersRequest>,
    ) -> Result<Response<QueryUsersResponse>, Status> {
        let query = request.into_inner().query;
        let query = query.map(|query| try_parse_user_query(query)).transpose()?;

        let users = self
            .store
            .query(query.as_ref())
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

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
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        Ok(Response::new(DeleteUsersResponse {}))
    }
}
