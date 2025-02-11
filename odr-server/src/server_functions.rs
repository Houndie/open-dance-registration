pub mod authentication;
pub mod event;
pub mod organization;
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

fn status_to_server_fn_error(status: tonic::Status) -> dioxus::prelude::ServerFnError {
    dioxus::prelude::ServerFnError::ServerError(status.message().to_string())
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
