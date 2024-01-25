use std::sync::Arc;

use argon2::{Argon2, PasswordVerifier};
use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{
    keys::KeyManager,
    store::{
        keys::Store as KeyStore,
        user::{
            CompoundOperator, CompoundQuery, EmailQuery, PasswordType, Query, Store as UserStore,
        },
    },
};

#[derive(Deserialize)]
enum LoginRequest {
    Credientials { email: String, password: String },
    Cookie,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

#[derive(Serialize)]
struct Claims {
    sub: String,
    kid: String,
}

fn api_routes<KStore: KeyStore, UStore: UserStore>(
    km: Arc<KeyManager<KStore>>,
    user_store: Arc<UStore>,
) -> Router {
    Router::new().route(
        "/login",
        post(|Json(request): Json<LoginRequest>| async move {
            match request {
                LoginRequest::Credientials { email, password } => {
                    let users = user_store
                        .query(Query::CompoundQuery(CompoundQuery {
                            operator: CompoundOperator::And,
                            queries: vec![
                                Query::Email(EmailQuery::Is(email)),
                                Query::PasswordIsSet(true),
                            ],
                        }))
                        .await
                        .map_err(|e| e.into_response())?;

                    if users.is_empty() {
                        return Err(
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        );
                    }

                    let user = &users[0];
                    let user_password = match &user.password {
                        PasswordType::Set(password) => password,
                        _ => {
                            return Err((StatusCode::UNAUTHORIZED, "invalid email or password")
                                .into_response())
                        }
                    };

                    Argon2::default()
                        .verify_password(password.as_bytes(), &user_password.password_hash())
                        .map_err(|_| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;
                }
                LoginRequest::Cookie => todo!(),
            };

            Ok(())
        }),
    )
}
