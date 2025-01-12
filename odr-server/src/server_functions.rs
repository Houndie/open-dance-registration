pub mod event;
pub mod organization;

use prost;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug)]
pub struct ProtoWrapper<T: prost::Message + Default>(pub T);

impl<T: prost::Message + Default> Serialize for ProtoWrapper<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let bytes = self.0.encode_to_vec();
        bytes.serialize(serializer)
    }
}

impl<'de, T: prost::Message + Default> Deserialize<'de> for ProtoWrapper<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = Vec::deserialize(deserializer)?;
        let message = T::decode(&*bytes).map_err(serde::de::Error::custom)?;
        Ok(ProtoWrapper(message))
    }
}

#[cfg(feature = "server")]
fn status_to_server_fn_error(status: tonic::Status) -> dioxus::prelude::ServerFnError {
    dioxus::prelude::ServerFnError::ServerError(status.message().to_string())
}
