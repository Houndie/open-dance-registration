use crate::{keys::KeyManager, proto, store};
use cookie::Cookie;
use http::header::COOKIE;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
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

pub async fn claims_from_request<KM: KeyManager, R>(
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

#[derive(Clone, Debug, Default)]
pub struct Claims {
    pub iss: String,
    pub sub: String,
    pub aud: Audience,
    pub iat: chrono::DateTime<chrono::Utc>,
    pub exp: chrono::DateTime<chrono::Utc>,
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
}
