use pin_project::pin_project;
use std::{
    future::Future as StdFuture,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer as TowerLayer, Service};

#[derive(Clone)]
pub struct Middleware<M, NM, const P: usize> {
    middleware: M,
    no_middleware: NM,
    omit: [&'static str; P],
}

impl<M, NM, RequestBody, const P: usize> Service<http::Request<RequestBody>>
    for Middleware<M, NM, P>
where
    NM: Service<http::Request<RequestBody>>,
    M: Service<
        http::Request<RequestBody>,
        Response = <NM as Service<http::Request<RequestBody>>>::Response,
        Error = <NM as Service<http::Request<RequestBody>>>::Error,
    >,
{
    type Response = <M as Service<http::Request<RequestBody>>>::Response;
    type Error = <M as Service<http::Request<RequestBody>>>::Error;
    type Future = Future<M, NM, RequestBody>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // We assume that the middleware is checking the readiness of the inner service
        self.middleware.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<RequestBody>) -> Self::Future {
        let path = req.uri().path().to_string();

        if self.omit.iter().any(|p| path.contains(p)) {
            Future::NM(self.no_middleware.call(req))
        } else {
            Future::M(self.middleware.call(req))
        }
    }
}

#[pin_project(project = SelectiveMiddlewareFutureProj)]
pub enum Future<M, NM, RequestBody>
where
    M: Service<http::Request<RequestBody>>,
    NM: Service<http::Request<RequestBody>>,
{
    M(#[pin] M::Future),
    NM(#[pin] NM::Future),
}

impl<M, NM, RequestBody> StdFuture for Future<M, NM, RequestBody>
where
    NM: Service<http::Request<RequestBody>>,
    M: Service<
        http::Request<RequestBody>,
        Response = <NM as Service<http::Request<RequestBody>>>::Response,
        Error = <NM as Service<http::Request<RequestBody>>>::Error,
    >,
{
    type Output = Result<NM::Response, NM::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            SelectiveMiddlewareFutureProj::M(future) => future.poll(cx),
            SelectiveMiddlewareFutureProj::NM(future) => future.poll(cx),
        }
    }
}

#[derive(Clone)]
pub struct Layer<ML, const P: usize> {
    omit: [&'static str; P],
    middleware_layer: ML,
}

impl<ML, const P: usize> Layer<ML, P> {
    pub fn new(middleware_layer: ML, omit: [&'static str; P]) -> Self {
        Self {
            middleware_layer,
            omit,
        }
    }
}

impl<NM: Clone, ML: Clone + TowerLayer<NM>, const P: usize> TowerLayer<NM> for Layer<ML, P> {
    type Service = Middleware<ML::Service, NM, P>;

    fn layer(&self, inner: NM) -> Self::Service {
        Middleware {
            middleware: self.middleware_layer.layer(inner.clone()),
            no_middleware: inner,
            omit: self.omit.clone(),
        }
    }
}
