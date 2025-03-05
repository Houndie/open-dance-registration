pub mod authentication;
pub mod event;
pub mod organization;
pub mod permission;
pub mod registration;
pub mod registration_schema;
pub mod user;

use prost;
use serde::{Deserialize, Serialize};
use thiserror;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtoWrapper<T: prost::Message + Default>(#[serde(with = "proto_wrapper")] pub T);

mod proto_wrapper {
    use serde::{Deserializer, Serializer};

    pub fn serialize<T: prost::Message, S: Serializer>(
        message: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let bytes = message.encode_to_vec();
        serde_bytes::serialize(&bytes, serializer)
    }

    pub fn deserialize<'de, T: prost::Message + Default, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        T::decode(&*(bytes)).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "web")]
fn wasm_client() -> tonic_web_wasm_client::Client {
    tonic_web_wasm_client::Client::new_with_options(
        "http://localhost:50051".to_string(),
        tonic_web_wasm_client::options::FetchOptions::new()
            .credentials(tonic_web_wasm_client::options::Credentials::Include),
    )
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum Error {
    #[error("service not found in server context")]
    ServiceNotInContext,

    #[error("error calling grpc function: {0}")]
    GrpcError(tonic::Status),
}

#[cfg(feature = "server")]
mod server_only {
    use super::Error;
    use crate::{
        api::authentication_middleware::verify_authentication_header,
        keys::{KeyManager, StoreKeyManager},
        store::{self, keys::SqliteStore as KeySqliteStore},
    };
    use dioxus::prelude::*;
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use std::sync::Arc;

    pub async fn tonic_request<T>(request: T) -> Result<tonic::Request<T>, Error> {
        let tonic_request = tonic_unauthenticated_request(request)?;

        let key_manager: AnyKeyManager = extract::<FromContext<AnyKeyManager>, _>()
            .await
            .map_err(|_| Error::ServiceNotInContext)?
            .0;

        let tonic_request = verify_authentication_header(&key_manager, tonic_request)
            .await
            .map_err(Error::GrpcError)?;

        Ok(tonic_request)
    }

    pub fn tonic_unauthenticated_request<T>(request: T) -> Result<tonic::Request<T>, Error> {
        let server_context = server_context();

        let mut tonic_request = tonic::Request::new(request);
        *tonic_request.metadata_mut() = tonic::metadata::MetadataMap::from_headers(
            server_context.request_parts().headers.clone(),
        );

        Ok(tonic_request)
    }

    pub fn tonic_response<T>(mut response: tonic::Response<T>) -> T {
        let server_context = server_context();
        let metadata = std::mem::take(response.metadata_mut());
        server_context
            .response_parts_mut()
            .headers
            .extend(metadata.into_headers());

        response.into_inner()
    }

    #[derive(Clone)]
    pub enum AnyKeyManager {
        Sqlite(Arc<StoreKeyManager<KeySqliteStore>>),
    }

    impl AnyKeyManager {
        pub fn new_sqlite(store: Arc<StoreKeyManager<KeySqliteStore>>) -> Self {
            AnyKeyManager::Sqlite(store)
        }
    }

    impl KeyManager for AnyKeyManager {
        async fn rotate_key(&self, clear: bool) -> Result<(), store::Error> {
            match self {
                AnyKeyManager::Sqlite(store) => store.rotate_key(clear).await,
            }
        }

        async fn get_signing_key(&self) -> Result<(String, SigningKey), store::Error> {
            match self {
                AnyKeyManager::Sqlite(store) => store.get_signing_key().await,
            }
        }

        async fn get_verifying_key(&self, kid: &str) -> Result<VerifyingKey, store::Error> {
            match self {
                AnyKeyManager::Sqlite(store) => store.get_verifying_key(kid).await,
            }
        }
    }
}

#[cfg(feature = "server")]
pub use server_only::*;
