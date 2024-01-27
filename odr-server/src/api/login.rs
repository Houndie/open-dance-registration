use std::sync::Arc;

use argon2::{Argon2, PasswordVerifier};
use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, EncodingKey, Validation};
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

#[derive(Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    iat: chrono::DateTime<chrono::Utc>,
    exp: chrono::DateTime<chrono::Utc>,
}

pub fn api_routes<KStore: KeyStore, UStore: UserStore>(
    km: Arc<KeyManager<KStore>>,
    user_store: Arc<UStore>,
) -> Router {
    Router::new().route(
        "/login",
        post(
            |jar: CookieJar, Json(request): Json<LoginRequest>| async move {
                let user_email = match request {
                    LoginRequest::Credientials { email, password } => {
                        let mut users = user_store
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
                            return Err((StatusCode::UNAUTHORIZED, "invalid email or password")
                                .into_response());
                        }

                        let user = users.remove(0);
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
                                (StatusCode::UNAUTHORIZED, "invalid email or password")
                                    .into_response()
                            })?;

                        user.email
                    }
                    LoginRequest::Cookie => {
                        let cookie = jar.get("authentication").ok_or_else(|| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;

                        let expiration = cookie.expires_datetime().ok_or_else(|| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;

                        if expiration.unix_timestamp() < chrono::Utc::now().timestamp() {
                            return Err((StatusCode::UNAUTHORIZED, "invalid email or password")
                                .into_response());
                        }

                        let token = cookie.value();

                        let header = decode_header(token).map_err(|_| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;

                        let kid = header.kid.ok_or_else(|| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;

                        let key = km.get_verifying_key(&kid).await.map_err(|_| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;

                        let decoding_key = DecodingKey::from_ed_der(
                            key.to_public_key_der()
                                .map_err(|_| {
                                    (StatusCode::UNAUTHORIZED, "invalid email or password")
                                        .into_response()
                                })?
                                .as_bytes(),
                        );

                        let token_data = decode::<Claims>(
                            token,
                            &decoding_key,
                            &Validation::new(Algorithm::EdDSA),
                        )
                        .map_err(|_| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?;

                        token_data.claims.sub
                    }
                };

                let claims = Claims {
                    iss: "https://auth.example.com".to_string(),
                    sub: user_email,
                    iat: chrono::Utc::now(),
                    exp: chrono::Utc::now() + chrono::Duration::weeks(26),
                };

                let (kid, key) = km.get_signing_key().await.map_err(|_| {
                    (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                })?;

                let encoding_key = EncodingKey::from_ed_der(
                    key.to_pkcs8_der()
                        .map_err(|_| {
                            (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                        })?
                        .as_bytes(),
                );

                let mut header = jsonwebtoken::Header::new(Algorithm::EdDSA);
                header.kid = Some(kid);

                let jwt = jsonwebtoken::encode(
                    &jsonwebtoken::Header::new(Algorithm::EdDSA),
                    &claims,
                    &encoding_key,
                )
                .map_err(|_| {
                    (StatusCode::UNAUTHORIZED, "invalid email or password").into_response()
                })?;

                let claims_cookie = Cookie::build(("authentication", jwt.clone()))
                    .secure(true)
                    .http_only(true)
                    .same_site(SameSite::Strict);

                Ok((
                    StatusCode::OK,
                    [("Set-Cookie", format!("{}", claims_cookie))],
                    Json(LoginResponse { token: jwt }),
                ))
            },
        ),
    )
}
