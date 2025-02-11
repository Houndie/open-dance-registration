use super::store_error_to_status;
use argon2::{Argon2, PasswordVerifier};
use common::proto::{self, ClaimsRequest, ClaimsResponse, LoginRequest, LoginResponse};
use cookie::{Cookie, CookieBuilder, Expiration, SameSite};
use ed25519_dalek::pkcs8::EncodePrivateKey;
use http::header::{HeaderMap, COOKIE, SET_COOKIE};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, EncodingKey, Validation};
use odr_core::{
    keys::KeyManager,
    store::{
        self,
        keys::Store as KeyStore,
        user::{EmailQuery, PasswordType, Query, Store as UserStore},
        CompoundOperator, CompoundQuery,
    },
};
use prost_types::Timestamp;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;
use tonic::{metadata::MetadataMap, Code, Request, Response, Status};

#[derive(Debug)]
struct Claims {
    iss: String,
    sub: String,
    aud: Audience,
    iat: chrono::DateTime<chrono::Utc>,
    exp: chrono::DateTime<chrono::Utc>,
}

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

#[derive(Debug, Serialize, Deserialize, strum::Display)]
enum Audience {
    Access,
}

impl From<Audience> for proto::Audience {
    fn from(aud: Audience) -> Self {
        match aud {
            Audience::Access => proto::Audience::Access,
        }
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
impl<KStore: KeyStore, UStore: UserStore>
    proto::authentication_service_server::AuthenticationService for Service<KStore, UStore>
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
                        Query::Email(EmailQuery::Equals(credentials.email)),
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
        mut request: Request<ClaimsRequest>,
    ) -> Result<Response<ClaimsResponse>, Status> {
        let metadata = std::mem::take(request.metadata_mut());
        let headers = metadata.into_headers();
        let cookies = headers.get_all(COOKIE);
        let auth_cookie = cookies.iter().find_map(|cookie_header_value| {
            let parsed = Cookie::parse(cookie_header_value.to_str().ok()?).ok()?;

            if parsed.name() != ACCESS_TOKEN_COOKIE {
                return None;
            }

            Some(parsed)
        });

        let auth_cookie = match auth_cookie {
            Some(cookie) => cookie,
            None => {
                return Err(ValidationError::Unauthenticated.into());
            }
        };

        let token = validate_token(&self.km, auth_cookie.value())
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(ClaimsResponse {
            claims: Some(token.into()),
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

#[derive(thiserror::Error, Debug)]
enum ValidationError {
    #[error("unauthenticated")]
    Unauthenticated,

    #[error(transparent)]
    StoreError(store::Error),
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

async fn validate_token<KStore: KeyStore>(
    km: &KeyManager<KStore>,
    token: &str,
) -> Result<Claims, ValidationError> {
    let header = decode_header(token).map_err(|_| ValidationError::Unauthenticated)?;

    let kid = header.kid.ok_or(ValidationError::Unauthenticated)?;

    let key = match km.get_verifying_key(&kid).await {
        Ok(key) => key,
        Err(store::Error::IdDoesNotExist(_)) => {
            return Err(ValidationError::Unauthenticated);
        }
        Err(e) => {
            return Err(ValidationError::StoreError(e));
        }
    };

    let decoding_key = DecodingKey::from_ed_der(key.to_bytes().as_slice());

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_audience(&[Audience::Access]);
    validation.set_issuer(&[ISSUER]);
    validation.validate_exp = true;

    let claims = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|_| ValidationError::Unauthenticated)?;

    Ok(claims.claims)
}
