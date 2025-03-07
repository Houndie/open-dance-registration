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
    use dioxus::prelude::*;
    use std::{future::Future, pin::Pin};
    use tonic::body::BoxBody;
    use tower::Service;

    pub fn tonic_request<T>(request: T) -> Result<tonic::Request<T>, Error> {
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

    trait WrappedService<S>: Service<S> + ClonedBoxService<S> {}

    trait ClonedBoxService<S>
    where
        Self: Service<S>,
    {
        fn clone_box(
            &self,
        ) -> Box<
            dyn WrappedService<
                    S,
                    Error = <Self as Service<S>>::Error,
                    Future = <Self as Service<S>>::Future,
                    Response = <Self as Service<S>>::Response,
                > + Send
                + Sync,
        >;
    }

    impl<T, S> ClonedBoxService<S> for T
    where
        T: WrappedService<S> + Clone + Send + Sync + 'static,
    {
        fn clone_box(
            &self,
        ) -> Box<
            dyn WrappedService<
                    S,
                    Error = <Self as Service<S>>::Error,
                    Future = <Self as Service<S>>::Future,
                    Response = <Self as Service<S>>::Response,
                > + Send
                + Sync,
        > {
            Box::new(self.clone())
        }
    }

    impl<S, E, F, R> Clone
        for Box<dyn WrappedService<S, Error = E, Future = F, Response = R> + Send + Sync>
    where
        F: Future<Output = Result<R, E>> + Send,
    {
        fn clone(&self) -> Self {
            self.clone_box()
        }
    }

    #[derive(Clone)]
    pub struct InternalServer {
        service: Box<
            dyn WrappedService<
                    http::request::Request<BoxBody>,
                    Response = http::response::Response<BoxBody>,
                    Error = Box<dyn std::error::Error + Sync + Send>,
                    Future = Pin<
                        Box<
                            dyn Future<
                                    Output = Result<
                                        http::response::Response<BoxBody>,
                                        Box<dyn std::error::Error + Sync + Send>,
                                    >,
                                > + Send,
                        >,
                    >,
                > + Send
                + Sync,
        >,
    }

    impl InternalServer {
        pub fn new<S>(service: S) -> Self
        where
            S: Service<
                    http::request::Request<BoxBody>,
                    Error = Box<dyn std::error::Error + Sync + Send>,
                    Response = http::response::Response<BoxBody>,
                > + Send
                + Sync
                + Clone
                + 'static,
            <S as Service<http::request::Request<BoxBody>>>::Future: Send,
        {
            Self {
                service: Box::new(InternalServerInner { service }),
            }
        }
    }

    impl Service<http::request::Request<BoxBody>> for InternalServer {
        type Response = http::response::Response<BoxBody>;
        type Error = Box<dyn std::error::Error + Sync + Send>;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            self.service.poll_ready(cx)
        }

        fn call(&mut self, req: http::request::Request<BoxBody>) -> Self::Future {
            self.service.call(req)
        }
    }

    #[derive(Clone)]
    struct InternalServerInner<S>
    where
        S: Service<
                http::request::Request<BoxBody>,
                Error = Box<dyn std::error::Error + Sync + Send>,
                Response = http::response::Response<BoxBody>,
            > + Send
            + Sync
            + Clone,
    {
        service: S,
    }

    impl<S> Service<http::request::Request<BoxBody>> for InternalServerInner<S>
    where
        S: Service<
                http::request::Request<BoxBody>,
                Error = Box<dyn std::error::Error + Sync + Send>,
                Response = http::response::Response<BoxBody>,
            > + Send
            + Sync
            + Clone
            + 'static,
        <S as Service<http::request::Request<BoxBody>>>::Future: Send,
    {
        type Response = http::response::Response<BoxBody>;
        type Error = Box<dyn std::error::Error + Sync + Send>;
        type Future =
            Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

        fn poll_ready(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            self.service
                .poll_ready(cx)
                .map(|result| result.map_err(|e| -> Self::Error { e }))
        }

        fn call(&mut self, req: http::request::Request<BoxBody>) -> Self::Future {
            let mut service = self.service.clone();
            Box::pin(async move {
                let response = service.call(req).await.map_err(|e| -> Self::Error { e })?;

                Ok(response)
            })
        }
    }

    impl<S> WrappedService<http::request::Request<BoxBody>> for InternalServerInner<S>
    where
        S: Service<
                http::request::Request<BoxBody>,
                Error = Box<dyn std::error::Error + Sync + Send>,
                Response = http::response::Response<BoxBody>,
            > + Send
            + Sync
            + Clone
            + 'static,
        <S as Service<http::request::Request<BoxBody>>>::Future: Send,
    {
    }
}

#[cfg(feature = "server")]
pub use server_only::*;
