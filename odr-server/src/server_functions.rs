pub mod event;
pub mod organization;

use dioxus::logger::tracing;
use prost;
use serde::{Deserialize, Serialize};

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

#[cfg(feature = "server")]
fn status_to_server_fn_error(status: tonic::Status) -> dioxus::prelude::ServerFnError {
    dioxus::prelude::ServerFnError::ServerError(status.message().to_string())
}
