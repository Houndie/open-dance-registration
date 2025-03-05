use crate::{
    api::store_error_to_status,
    authentication::{
        claims_from_request, Audience, Claims, ValidationError, ACCESS_TOKEN_COOKIE, ISSUER,
    },
    keys::KeyManager,
    proto::{self, ClaimsRequest, ClaimsResponse, LoginRequest, LoginResponse},
    store::{
        self,
        user::{PasswordType, Query, Store as UserStore, UsernameQuery},
        CompoundOperator, CompoundQuery,
    },
};
use argon2::{Argon2, PasswordVerifier};
use cookie::{Cookie, CookieBuilder, Expiration, SameSite};
use ed25519_dalek::pkcs8::EncodePrivateKey;
use http::header::{HeaderMap, SET_COOKIE};
use jsonwebtoken::{Algorithm, EncodingKey};
use prost_types::Timestamp;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;
use tonic::{metadata::MetadataMap, Code, Request, Response, Status};

impl From<Claims> for proto::Claims {
    fn from(claims: Claims) -> Self {
        proto::Claims {
            iss: claims.iss,
            sub: claims.sub,
            aud: Into::<proto::Audience>::into(claims.aud) as i32,
            iat: Some(Timestamp {
                seconds: claims.iat.timestamp(),
                nanos: 0,
            }),
            exp: Some(Timestamp {
                seconds: claims.exp.timestamp(),
                nanos: 0,
            }),
        }
    }
}

impl Serialize for Claims {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct SerializeClaims<'a> {
            iss: &'a str,
            sub: &'a str,
            aud: &'a Audience,
            iat: i64,
            exp: i64,
        }

        let claims = SerializeClaims {
            iss: &self.iss,
            sub: &self.sub,
            aud: &self.aud,
            iat: self.iat.timestamp(),
            exp: self.exp.timestamp(),
        };

        claims.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Claims {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DeserializeClaims {
            iss: String,
            sub: String,
            aud: Audience,
            iat: i64,
            exp: i64,
        }

        let claims = DeserializeClaims::deserialize(deserializer)?;

        Ok(Claims {
            iss: claims.iss,
            sub: claims.sub,
            aud: claims.aud,
            iat: chrono::DateTime::<chrono::Utc>::from_timestamp(claims.iat, 0)
                .ok_or_else(|| serde::de::Error::custom("invalid timestamp"))?,
            exp: chrono::DateTime::<chrono::Utc>::from_timestamp(claims.exp, 0)
                .ok_or_else(|| serde::de::Error::custom("invalid timestamp"))?,
        })
    }
}

const ACCESS_TOKEN_EXPIRATION_SECONDS: i64 = 60 * 60 * 24 * 30 * 6;

pub struct Service<KM: KeyManager, UStore: UserStore> {
    km: Arc<KM>,
    user_store: Arc<UStore>,
}

impl<KM: KeyManager, UStore: UserStore> Service<KM, UStore> {
    pub fn new(km: Arc<KM>, user_store: Arc<UStore>) -> Self {
        Self { km, user_store }
    }
}

#[tonic::async_trait]
impl<KM: KeyManager, UStore: UserStore> proto::authentication_service_server::AuthenticationService
    for Service<KM, UStore>
{
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let credentials = request.into_inner();

        let invalid_email_or_password =
            || Status::new(Code::Unauthenticated, "Invalid email or password");

        let mut users = self
            .user_store
            .query(
                Some(Query::CompoundQuery(CompoundQuery {
                    operator: CompoundOperator::And,
                    queries: vec![
                        Query::Username(UsernameQuery::Equals(credentials.email)),
                        Query::PasswordIsSet(true),
                    ],
                }))
                .as_ref(),
            )
            .await
            .map_err(|e| match e {
                store::Error::IdDoesNotExist(_) => invalid_email_or_password(),
                _ => store_error_to_status(e),
            })?;

        if users.is_empty() {
            return Err(invalid_email_or_password());
        }

        let user = users.remove(0);
        let user_password = match &user.password {
            PasswordType::Set(password) => password,
            _ => {
                return Err(invalid_email_or_password());
            }
        };

        Argon2::default()
            .verify_password(
                credentials.password.as_bytes(),
                &user_password.password_hash(),
            )
            .map_err(|_| invalid_email_or_password())?;

        let (kid, key) = self
            .km
            .get_signing_key()
            .await
            .map_err(|e| -> Status { store_error_to_status(e) })?;

        let encoding_key = EncodingKey::from_ed_der(
            key.to_pkcs8_der()
                .map_err(|e| {
                    Status::new(
                        Code::Internal,
                        format!("error generating signing key {}", e),
                    )
                })?
                .as_bytes(),
        );

        let mut header = jsonwebtoken::Header::new(Algorithm::EdDSA);
        header.kid = Some(kid);

        let access_claims = Claims {
            iss: ISSUER.to_string(),
            sub: user.id,
            aud: Audience::Access,
            iat: chrono::Utc::now(),
            exp: chrono::Utc::now() + chrono::Duration::seconds(ACCESS_TOKEN_EXPIRATION_SECONDS),
        };

        let access_jwt = jsonwebtoken::encode(&header, &access_claims, &encoding_key)
            .map_err(|e| Status::new(Code::Internal, format!("error signing token: {}", e)))?;

        let claims_cookie = Cookie::build((ACCESS_TOKEN_COOKIE, access_jwt.clone()))
            .expires(Expiration::DateTime(
                time::OffsetDateTime::from_unix_timestamp(access_claims.exp.timestamp()).unwrap(),
            ))
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict)
            .path("/");

        let mut response = Response::new(LoginResponse {
            claims: Some(access_claims.into()),
        });
        let mut metadata = HeaderMap::new();
        metadata.insert(SET_COOKIE, claims_cookie.to_string().parse().unwrap());
        *response.metadata_mut() = MetadataMap::from_headers(metadata);

        Ok(response)
    }

    async fn claims(
        &self,
        request: Request<ClaimsRequest>,
    ) -> Result<Response<ClaimsResponse>, Status> {
        let claims = claims_from_request(&*self.km, &request).await?;

        Ok(Response::new(ClaimsResponse {
            claims: Some(claims.into()),
        }))
    }

    async fn logout(
        &self,
        _request: Request<proto::LogoutRequest>,
    ) -> Result<Response<proto::LogoutResponse>, Status> {
        let mut response = Response::new(proto::LogoutResponse {});
        let mut metadata = HeaderMap::new();
        metadata.insert(SET_COOKIE, delete_cookie().to_string().parse().unwrap());
        *response.metadata_mut() = MetadataMap::from_headers(metadata);

        Ok(response)
    }
}

fn delete_cookie() -> CookieBuilder<'static> {
    Cookie::build((ACCESS_TOKEN_COOKIE, ""))
        .expires(Expiration::DateTime(OffsetDateTime::UNIX_EPOCH))
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
}

impl From<ValidationError> for Status {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::Unauthenticated => {
                Status::new(Code::Unauthenticated, "unauthenticated")
            }
            ValidationError::StoreError(e) => store_error_to_status(e),
        }
    }
}
