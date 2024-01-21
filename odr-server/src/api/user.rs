use std::sync::Arc;

use common::proto::{
    self, user::Password, DeleteUsersRequest, DeleteUsersResponse, ListUsersRequest,
    ListUsersResponse, UpsertUsersRequest, UpsertUsersResponse,
};
use tonic::{Request, Response, Status};

use crate::store::user::{self as store, HashedPassword, PasswordType, Store};

use super::{store_error_to_status, ValidationError};

pub struct Service<StoreType: Store> {
    store: Arc<StoreType>,
}

impl<StoreType: Store> Service<StoreType> {
    pub fn new(store: Arc<StoreType>) -> Self {
        Service { store }
    }
}

fn proto_to_user(proto_user: proto::User) -> Result<store::User, Status> {
    let password = match proto_user.password.unwrap() {
        Password::Set(password) => {
            let hashed_password = HashedPassword::new(password)
                .map_err(|e| Status::internal(format!("unable to hash password: {}", e)))?;

            PasswordType::Set(hashed_password)
        }
        Password::Unset(_) => PasswordType::Unset,
        Password::Unchanged(_) => PasswordType::Unchanged,
    };

    Ok(store::User {
        id: proto_user.id,
        email: proto_user.email,
        password,
        display_name: proto_user.display_name,
    })
}

fn user_to_proto(user: store::User) -> proto::User {
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

    async fn list_users(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let users = self
            .store
            .list(request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(ListUsersResponse {
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
