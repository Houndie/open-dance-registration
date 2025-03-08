use crate::{
    keys::KeyManager,
    proto,
    store::{
        self,
        user::{PasswordType, Query, Store as UserStore, UsernameQuery},
        CompoundOperator, CompoundQuery,
    },
};
use argon2::{Argon2, PasswordVerifier};
use cookie::Cookie;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use http::header::{AUTHORIZATION, COOKIE};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};

pub const ISSUER: &str = "https://auth.example.com";
pub const ACCESS_TOKEN_COOKIE: &str = "authorization";

pub async fn validate_token<KM: KeyManager>(
    km: &KM,
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

pub async fn claims_from_request_cookie<KM: KeyManager, R>(
    key_manager: &KM,
    request: &tonic::Request<R>,
) -> Result<Claims, ValidationError> {
    let metadata = request.metadata();
    let cookies = metadata.get_all(COOKIE.as_str());
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

    validate_token(&*key_manager, auth_cookie.value()).await
}

pub async fn claims_from_request_header<KM: KeyManager, R>(
    key_manager: &KM,
    request: &tonic::Request<R>,
) -> Result<Claims, ValidationError> {
    let metadata = request.metadata();
    let auth_header = metadata
        .get(AUTHORIZATION.as_str())
        .ok_or(ValidationError::Unauthenticated)?
        .to_str()
        .map_err(|_| ValidationError::Unauthenticated)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(ValidationError::Unauthenticated)?;

    validate_token(&*key_manager, token).await
}

#[derive(Clone, Debug, Default)]
pub struct Claims {
    pub iss: String,
    pub sub: String,
    pub aud: Audience,
    pub iat: chrono::DateTime<chrono::Utc>,
    pub exp: chrono::DateTime<chrono::Utc>,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, strum::Display)]
pub enum Audience {
    #[default]
    Access,
}

impl From<Audience> for proto::Audience {
    fn from(aud: Audience) -> Self {
        match aud {
            Audience::Access => proto::Audience::Access,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("unauthenticated")]
    Unauthenticated,

    #[error(transparent)]
    StoreError(store::Error),

    #[error("error generating encoding key: {0}")]
    EncodingKeyError(ed25519_dalek::pkcs8::Error),

    #[error("error signing token: {0}")]
    SigningError(jsonwebtoken::errors::Error),
}

const ACCESS_TOKEN_EXPIRATION_SECONDS: i64 = 60 * 60 * 24 * 30 * 6;

pub async fn create_token<KM: KeyManager, UStore: UserStore>(
    km: &KM,
    user_store: &UStore,
    username: String,
    password: &str,
) -> Result<(Claims, String), ValidationError> {
    let mut users = user_store
        .query(
            Some(Query::CompoundQuery(CompoundQuery {
                operator: CompoundOperator::And,
                queries: vec![
                    Query::Username(UsernameQuery::Equals(username)),
                    Query::PasswordIsSet(true),
                ],
            }))
            .as_ref(),
        )
        .await
        .map_err(|e| match e {
            store::Error::IdDoesNotExist(_) => ValidationError::Unauthenticated,
            _ => ValidationError::StoreError(e),
        })?;

    if users.is_empty() {
        return Err(ValidationError::Unauthenticated);
    }

    let user = users.remove(0);
    let user_password = match &user.password {
        PasswordType::Set(password) => password,
        _ => {
            return Err(ValidationError::Unauthenticated);
        }
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &user_password.password_hash())
        .map_err(|_| ValidationError::Unauthenticated)?;

    let (kid, key) = km
        .get_signing_key()
        .await
        .map_err(ValidationError::StoreError)?;

    let encoding_key = EncodingKey::from_ed_der(
        key.to_pkcs8_der()
            .map_err(ValidationError::EncodingKeyError)?
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
        .map_err(ValidationError::SigningError)?;

    Ok((access_claims, access_jwt))
}
