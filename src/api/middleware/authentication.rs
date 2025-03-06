use crate::{
    authentication::{claims_from_request, Claims},
    keys::KeyManager,
};
use std::{future::Future, pin::Pin, sync::Arc};
use tonic::{Request, Status};

pub struct Interceptor<KM: KeyManager> {
    key_manager: Arc<KM>,
}

impl<KM: KeyManager> Clone for Interceptor<KM> {
    fn clone(&self) -> Self {
        Self {
            key_manager: self.key_manager.clone(),
        }
    }
}

impl<KM: KeyManager> Interceptor<KM> {
    pub fn new(key_manager: Arc<KM>) -> Self {
        Self { key_manager }
    }
}

impl<KM: KeyManager> tonic_async_interceptor::AsyncInterceptor for Interceptor<KM> {
    type Future = Pin<Box<dyn Future<Output = Result<Request<()>, Status>> + Send>>;

    fn call(&mut self, request: Request<()>) -> Self::Future {
        let key_manager = self.key_manager.clone();
        Box::pin(async move { verify_authentication_header(&*key_manager, request).await })
    }
}

pub async fn verify_authentication_header<KM: KeyManager, R>(
    key_manager: &KM,
    mut request: Request<R>,
) -> Result<Request<R>, Status> {
    let claims = claims_from_request(key_manager, &request).await?;

    request.extensions_mut().insert(ClaimsContext { claims });

    Ok(request)
}

#[derive(Clone)]
pub struct ClaimsContext {
    pub claims: Claims,
}
