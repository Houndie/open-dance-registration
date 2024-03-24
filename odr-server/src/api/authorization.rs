use std::sync::Arc;

use argon2::{Argon2, PasswordVerifier};
use common::proto::{self, LoginRequest, LoginResponse};
use cookie::{Cookie, SameSite};
use ed25519_dalek::pkcs8::{self, EncodePrivateKey};
use http::header::{HeaderMap, SET_COOKIE};
use jsonwebtoken::{Algorithm, EncodingKey};
use serde::{Deserialize, Serialize};
use tonic::{metadata::MetadataMap, Code, Request, Response, Status};

use crate::{
    keys::KeyManager,
    store::{
        self,
        keys::Store as KeyStore,
        user::{EmailQuery, PasswordType, Query, Store as UserStore},
        CompoundOperator, CompoundQuery,
    },
};

use super::store_error_to_status;

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

    #[error("error generating signing key: {0}")]
    InvalidSigningKey(#[source] pkcs8::Error),

    #[error("error signing token: {0}")]
    SigningError(#[source] jsonwebtoken::errors::Error),

    #[error("{0}")]
    StoreError(#[source] store::Error),
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        let code = match err {
            Error::StoreError(e) => return store_error_to_status(e),
            Error::InvalidEmailOrPassword => Code::Unauthenticated,
            Error::InvalidSigningKey(_) | Error::SigningError(_) => Code::Internal,
        };

        Status::new(code, err.to_string())
    }
}

const ISSUER: &str = "https://auth.example.com";
const ACCESS_TOKEN_EXPIRATION_SECONDS: i64 = 60 * 60 * 24 * 30 * 6;
const ACCESS_TOKEN_COOKIE: &str = "authorization";

pub struct Service<KStore: KeyStore, UStore: UserStore> {
    km: Arc<KeyManager<KStore>>,
    user_store: Arc<UStore>,
}

impl<KStore: KeyStore, UStore: UserStore> Service<KStore, UStore> {
    pub fn new(km: Arc<KeyManager<KStore>>, user_store: Arc<UStore>) -> Self {
        Self { km, user_store }
    }
}

#[tonic::async_trait]
impl<KStore: KeyStore, UStore: UserStore> proto::authorization_service_server::AuthorizationService
    for Service<KStore, UStore>
{
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let credentials = request.into_inner();

        let mut users = self
            .user_store
            .query(
                Some(Query::CompoundQuery(CompoundQuery {
                    operator: CompoundOperator::And,
                    queries: vec![
                        Query::Email(EmailQuery::Equals(credentials.email)),
                        Query::PasswordIsSet(true),
                    ],
                }))
                .as_ref(),
            )
            .await
            .map_err(|e| match e {
                store::Error::IdDoesNotExist(_) => Error::InvalidEmailOrPassword,
                _ => Error::StoreError(e),
            })?;

        if users.is_empty() {
            return Err(Error::InvalidEmailOrPassword.into());
        }

        let user = users.remove(0);
        let user_password = match &user.password {
            PasswordType::Set(password) => password,
            _ => {
                return Err(Error::InvalidEmailOrPassword.into());
            }
        };

        Argon2::default()
            .verify_password(
                credentials.password.as_bytes(),
                &user_password.password_hash(),
            )
            .map_err(|_| -> Status { Error::InvalidEmailOrPassword.into() })?;

        let (kid, key) = self
            .km
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

        let access_claims = Claims {
            iss: ISSUER.to_string(),
            sub: user.email,
            aud: Audience::Access,
            iat: chrono::Utc::now(),
            exp: chrono::Utc::now() + chrono::Duration::seconds(ACCESS_TOKEN_EXPIRATION_SECONDS),
        };

        let access_jwt = jsonwebtoken::encode(&header, &access_claims, &encoding_key)
            .map_err(|e| Error::SigningError(e))?;

        let claims_cookie = Cookie::build((ACCESS_TOKEN_COOKIE, access_jwt.clone()))
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict);

        let mut response = Response::new(LoginResponse {});
        let mut metadata = HeaderMap::new();
        metadata.insert(SET_COOKIE, claims_cookie.to_string().parse().unwrap());
        *response.metadata_mut() = MetadataMap::from_headers(metadata);

        Ok(response)
    }

    async fn logout(
        &self,
        _request: Request<proto::LogoutRequest>,
    ) -> Result<Response<proto::LogoutResponse>, Status> {
        let response = Response::new(proto::LogoutResponse {});

        Ok(response)
    }
}
