use crate::{
    api::store_error_to_status,
    authentication::{
        claims_from_request_cookie, create_token, Claims, ValidationError, ACCESS_TOKEN_COOKIE,
    },
    keys::KeyManager,
    proto::{
        self, authentication_service_server::AuthenticationService,
        web_authentication_service_server::WebAuthenticationService, ClaimsRequest, ClaimsResponse,
        LoginRequest, LoginResponse, WebLoginRequest, WebLoginResponse,
    },
    store::user::Store as UserStore,
};
use cookie::{Cookie, CookieBuilder, Expiration, SameSite};
use http::header::{HeaderMap, SET_COOKIE};
use prost_types::Timestamp;
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

pub struct WebService<KM: KeyManager, UStore: UserStore> {
    km: Arc<KM>,
    user_store: Arc<UStore>,
}

impl<KM: KeyManager, UStore: UserStore> WebService<KM, UStore> {
    pub fn new(km: Arc<KM>, user_store: Arc<UStore>) -> Self {
        Self { km, user_store }
    }
}

#[tonic::async_trait]
impl<KM: KeyManager, UStore: UserStore> WebAuthenticationService for WebService<KM, UStore> {
    async fn login(
        &self,
        request: Request<WebLoginRequest>,
    ) -> Result<Response<WebLoginResponse>, Status> {
        let credentials = request.into_inner();

        let (access_claims, access_jwt) = create_token(
            &*self.km,
            &*self.user_store,
            credentials.username,
            credentials.password.as_str(),
        )
        .await?;

        let claims_cookie = Cookie::build((ACCESS_TOKEN_COOKIE, access_jwt))
            .expires(Expiration::DateTime(
                time::OffsetDateTime::from_unix_timestamp(access_claims.exp.timestamp()).unwrap(),
            ))
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict)
            .path("/");

        let mut response = Response::new(WebLoginResponse {
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
        let claims = claims_from_request_cookie(&*self.km, &request).await?;

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

pub struct HeaderService<KM: KeyManager, UStore: UserStore> {
    km: Arc<KM>,
    user_store: Arc<UStore>,
}

impl<KM: KeyManager, UStore: UserStore> HeaderService<KM, UStore> {
    pub fn new(km: Arc<KM>, user_store: Arc<UStore>) -> Self {
        Self { km, user_store }
    }
}

#[tonic::async_trait]
impl<KM: KeyManager, UStore: UserStore> AuthenticationService for HeaderService<KM, UStore> {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let credentials = request.into_inner();

        let (_, access_jwt) = create_token(
            &*self.km,
            &*self.user_store,
            credentials.username,
            credentials.password.as_str(),
        )
        .await?;

        let response = Response::new(LoginResponse { token: access_jwt });

        Ok(response)
    }
}

impl From<ValidationError> for Status {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::Unauthenticated => {
                Status::new(Code::Unauthenticated, "unauthenticated")
            }
            ValidationError::StoreError(e) => store_error_to_status(e),
            ValidationError::EncodingKeyError(_) | ValidationError::SigningError(_) => {
                Status::new(Code::Internal, format!("internal error: {}", err))
            }
        }
    }
}
