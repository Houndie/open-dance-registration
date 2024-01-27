use std::sync::Arc;

use argon2::{Argon2, PasswordVerifier};
use axum::{
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use ed25519_dalek::pkcs8::{self, EncodePrivateKey, EncodePublicKey};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::{
    keys::KeyManager,
    store::{
        self,
        keys::Store as KeyStore,
        user::{EmailQuery, PasswordType, Query, Store as UserStore},
        CompoundOperator, CompoundQuery,
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
    aud: Audience,
    iat: chrono::DateTime<chrono::Utc>,
    exp: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, strum::Display)]
enum Audience {
    Access,
    Refresh,
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("invalid email or password")]
    InvalidEmailOrPassword,

    #[error("invalid refresh token")]
    InvalidRefreshToken,

    #[error("error generating verification key: {0}")]
    InvalidVerificationKey(#[source] pkcs8::spki::Error),

    #[error("error generating signing key: {0}")]
    InvalidSigningKey(#[source] pkcs8::Error),

    #[error("error signing token: {0}")]
    SigningError(#[source] jsonwebtoken::errors::Error),

    #[error("{0}")]
    StoreError(#[source] store::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        let code = match self {
            Self::StoreError(e) => return e.into_response(),
            Self::InvalidEmailOrPassword | Self::InvalidRefreshToken => StatusCode::UNAUTHORIZED,
            Self::InvalidVerificationKey(_)
            | Self::InvalidSigningKey(_)
            | Self::SigningError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (code, self.to_string()).into_response()
    }
}

const ISSUER: &str = "https://auth.example.com";
const REFRESH_TOKEN_EXPIRATION_SECONDS: i64 = 60 * 60 * 24 * 7 * 26;
const ACCESS_TOKEN_EXPIRATION_SECONDS: i64 = 60 * 60;
const REFRESH_TOKEN_COOKIE: &str = "refresh_token";

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
                            .map_err(|e| match e {
                                store::Error::IdDoesNotExist(_) => Error::InvalidEmailOrPassword,
                                _ => Error::StoreError(e),
                            })?;

                        if users.is_empty() {
                            return Err(Error::InvalidEmailOrPassword);
                        }

                        let user = users.remove(0);
                        let user_password = match &user.password {
                            PasswordType::Set(password) => password,
                            _ => {
                                return Err(Error::InvalidEmailOrPassword);
                            }
                        };

                        Argon2::default()
                            .verify_password(password.as_bytes(), &user_password.password_hash())
                            .map_err(|_| Error::InvalidEmailOrPassword)?;

                        user.email
                    }
                    LoginRequest::Cookie => {
                        let cookie = jar
                            .get(REFRESH_TOKEN_COOKIE)
                            .ok_or_else(|| Error::InvalidRefreshToken)?;

                        let expiration = cookie
                            .expires_datetime()
                            .ok_or_else(|| Error::InvalidRefreshToken)?;

                        if expiration.unix_timestamp() < chrono::Utc::now().timestamp() {
                            return Err(Error::InvalidRefreshToken);
                        }

                        let token = cookie.value();

                        let header =
                            decode_header(token).map_err(|_| Error::InvalidRefreshToken)?;

                        let kid = header.kid.ok_or_else(|| Error::InvalidRefreshToken)?;

                        let key = km.get_verifying_key(&kid).await.map_err(|e| match e {
                            store::Error::IdDoesNotExist(_) => Error::InvalidRefreshToken,
                            _ => Error::StoreError(e),
                        })?;

                        let decoding_key = DecodingKey::from_ed_der(
                            key.to_public_key_der()
                                .map_err(|e| Error::InvalidVerificationKey(e))?
                                .as_bytes(),
                        );

                        let mut validation = Validation::new(Algorithm::EdDSA);
                        validation.set_audience(&[Audience::Refresh]);
                        validation.set_issuer(&[ISSUER]);

                        let token_data = decode::<Claims>(token, &decoding_key, &validation)
                            .map_err(|_| Error::InvalidRefreshToken)?;

                        // Make sure the user hasn't been removed since the token was issued
                        let mut users = user_store
                            .query(Query::Email(EmailQuery::Is(token_data.claims.sub)))
                            .await
                            .map_err(|e| match e {
                                store::Error::IdDoesNotExist(_) => Error::InvalidRefreshToken,
                                _ => Error::StoreError(e),
                            })?;

                        if users.is_empty() {
                            return Err(Error::InvalidRefreshToken);
                        }

                        users.remove(0).email
                    }
                };

                let (kid, key) = km
                    .get_signing_key()
                    .await
                    .map_err(|e| Error::StoreError(e))?;

                let encoding_key = EncodingKey::from_ed_der(
                    key.to_pkcs8_der()
                        .map_err(|e| Error::InvalidSigningKey(e))?
                        .as_bytes(),
                );

                let mut header = jsonwebtoken::Header::new(Algorithm::EdDSA);
                header.kid = Some(kid);

                let refresh_claims = Claims {
                    iss: ISSUER.to_string(),
                    sub: user_email.clone(),
                    aud: Audience::Refresh,
                    iat: chrono::Utc::now(),
                    exp: chrono::Utc::now()
                        + chrono::Duration::seconds(REFRESH_TOKEN_EXPIRATION_SECONDS),
                };

                let refresh_jwt = jsonwebtoken::encode(&header, &refresh_claims, &encoding_key)
                    .map_err(|e| Error::SigningError(e))?;

                let claims_cookie = Cookie::build((REFRESH_TOKEN_COOKIE, refresh_jwt.clone()))
                    .secure(true)
                    .http_only(true)
                    .same_site(SameSite::Strict);

                let access_claims = Claims {
                    iss: ISSUER.to_string(),
                    sub: user_email,
                    aud: Audience::Access,
                    iat: chrono::Utc::now(),
                    exp: chrono::Utc::now()
                        + chrono::Duration::seconds(ACCESS_TOKEN_EXPIRATION_SECONDS),
                };

                let access_jwt = jsonwebtoken::encode(&header, &access_claims, &encoding_key)
                    .map_err(|e| Error::SigningError(e))?;

                Ok((
                    StatusCode::OK,
                    [("Set-Cookie", format!("{}", claims_cookie))],
                    Json(LoginResponse { token: access_jwt }),
                ))
            },
        ),
    )
}
