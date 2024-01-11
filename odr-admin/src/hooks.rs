use std::sync::{Arc, Mutex};

use common::proto::event_service_client::EventServiceClient;
use dioxus::prelude::*;

pub mod toasts;

pub struct ClientContext<Client>(Arc<Mutex<Client>>);

pub struct EventsClient;

pub trait ClientTraits {
    type Client;
    type Context: 'static;

    fn new(addr: String) -> Self::Context;
    fn deref_context(ctx: &Self::Context) -> Arc<Mutex<Self::Client>>;
}

impl ClientTraits for EventsClient {
    type Client = EventServiceClient<tonic_web_wasm_client::Client>;
    type Context = ClientContext<Self::Client>;

    fn new(addr: String) -> Self::Context {
        ClientContext::<Self::Client>(Arc::new(Mutex::new(Self::Client::new(
            tonic_web_wasm_client::Client::new(addr),
        ))))
    }

    fn deref_context(ctx: &Self::Context) -> Arc<Mutex<Self::Client>> {
        ctx.0.clone()
    }
}

pub fn use_grpc_client_provider<Client: ClientTraits>(cx: &ScopeState) {
    use_shared_state_provider(cx, || Client::new("http://localhost:50051".to_owned()))
}

pub fn use_grpc_client<Client: ClientTraits>(
    cx: &ScopeState,
) -> Option<Arc<Mutex<Client::Client>>> {
    use_shared_state::<Client::Context>(cx).map(|state| Client::deref_context(&*state.read()))
}
