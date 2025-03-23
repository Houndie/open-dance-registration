#![cfg(feature = "server")]

use crate::{
    api::{
        authentication::{
            HeaderService as AuthenticationGRPCService, WebService as AuthenticationWebService,
        },
        event::Service as EventService,
        middleware::{
            authentication::{
                CookieInterceptor as AuthCookieInterceptor,
                HeaderInterceptor as AuthHeaderInterceptor,
            },
            selective::Layer as SelectiveMiddleware,
        },
        organization::Service as OrganizationService,
        permission::Service as PermissionService,
        registration::Service as RegistrationService,
        registration_schema::Service as SchemaService,
        user::Service as UserService,
    },
    keys::StoreKeyManager,
    proto::{
        self, authentication_service_server::AuthenticationServiceServer,
        event_service_server::EventServiceServer,
        organization_service_server::OrganizationServiceServer,
        permission_service_server::PermissionServiceServer,
        registration_schema_service_server::RegistrationSchemaServiceServer,
        registration_service_server::RegistrationServiceServer,
        user_service_server::UserServiceServer,
        web_authentication_service_server::WebAuthenticationServiceServer,
    },
    server_functions::InternalServer,
    store::{
        event::SqliteStore as EventStore, keys::SqliteStore as KeyStore,
        organization::SqliteStore as OrganizationStore, permission::SqliteStore as PermissionStore,
        registration::SqliteStore as RegistrationStore,
        registration_schema::SqliteStore as SchemaStore, user::SqliteStore as UserStore,
    },
};
use dioxus::prelude::{DioxusRouterExt, ServeConfig};
use sqlx::SqlitePool;
use std::{env, sync::Arc};
use thiserror::Error;
use tonic::{
    server::NamedService,
    transport::{self, Server},
};
use tonic_async_interceptor::async_interceptor;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, TraceLayer};
use tracing::Level;

fn db_url() -> String {
    format!("sqlite://{}/odr-sqlite.db", env::temp_dir().display())
}

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    dioxus::logger::init(Level::INFO).expect("Failed to initialize logger");

    let db_url = db_url();

    let db = Arc::new(SqlitePool::connect(&db_url).await?);

    let event_store = Arc::new(EventStore::new(db.clone()));
    let schema_store = Arc::new(SchemaStore::new(db.clone()));
    let registration_store = Arc::new(RegistrationStore::new(db.clone()));
    let organization_store = Arc::new(OrganizationStore::new(db.clone()));
    let user_store = Arc::new(UserStore::new(db.clone()));
    let key_store = Arc::new(KeyStore::new(db.clone()));
    let permission_store = Arc::new(PermissionStore::new(db.clone()));

    let key_manager = Arc::new(StoreKeyManager::new(key_store));

    let event_service = Arc::new(EventService::new(event_store));

    let schema_service = Arc::new(SchemaService::new(schema_store));

    let registration_service = Arc::new(RegistrationService::new(registration_store));

    let organization_service = Arc::new(OrganizationService::new(organization_store));

    let authentication_web_service = Arc::new(AuthenticationWebService::new(
        key_manager.clone(),
        user_store.clone(),
    ));

    let authentication_grpc_service = Arc::new(AuthenticationGRPCService::new(
        key_manager.clone(),
        user_store.clone(),
    ));

    let user_service = Arc::new(UserService::new(user_store, permission_store.clone()));

    let permission_service = Arc::new(PermissionService::new(permission_store));

    let event_grpc = EventServiceServer::from_arc(event_service.clone());

    let registration_schema_grpc =
        RegistrationSchemaServiceServer::from_arc(schema_service.clone());

    let registration_grpc = RegistrationServiceServer::from_arc(registration_service.clone());

    let organization_grpc = OrganizationServiceServer::from_arc(organization_service.clone());

    let user_grpc = UserServiceServer::from_arc(user_service.clone());

    let authentication_web_grpc =
        WebAuthenticationServiceServer::from_arc(authentication_web_service.clone());

    let authentication_grpc_grpc =
        AuthenticationServiceServer::from_arc(authentication_grpc_service.clone());

    let permission_grpc = PermissionServiceServer::from_arc(permission_service.clone());

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1alpha()?;

    let omit_authentication_paths = [service_name(&authentication_web_grpc), "grpc.reflection"];

    let auth_cookie_middleware = SelectiveMiddleware::new(
        async_interceptor(AuthCookieInterceptor::new(key_manager.clone())),
        omit_authentication_paths,
    );

    let auth_header_middleware = SelectiveMiddleware::new(
        async_interceptor(AuthHeaderInterceptor::new(key_manager.clone())),
        omit_authentication_paths,
    );

    let trace_middleware = TraceLayer::new_for_grpc()
        .make_span_with(DefaultMakeSpan::default().level(Level::INFO))
        .on_request(DefaultOnRequest::default().level(Level::INFO));

    let grpc_addr = "[::1]:50050".parse()?;

    let grpc_server = Server::builder()
        .layer(trace_middleware.clone())
        .layer(auth_header_middleware.clone())
        .add_service(event_grpc.clone())
        .add_service(registration_schema_grpc.clone())
        .add_service(registration_grpc.clone())
        .add_service(organization_grpc.clone())
        .add_service(user_grpc.clone())
        .add_service(authentication_grpc_grpc.clone())
        .add_service(permission_grpc.clone())
        .add_service(reflection_service)
        .serve(grpc_addr);

    let internal_server = Server::builder()
        .layer(auth_cookie_middleware.clone())
        .add_service(event_grpc.clone())
        .add_service(registration_schema_grpc.clone())
        .add_service(registration_grpc.clone())
        .add_service(organization_grpc.clone())
        .add_service(user_grpc.clone())
        .add_service(authentication_web_grpc.clone())
        .add_service(permission_grpc.clone())
        .into_service();

    let grpc_web_addr = "[::1]:50051".parse()?;

    let grpc_web_server = Server::builder()
        .layer(trace_middleware)
        .layer(auth_cookie_middleware)
        .accept_http1(true)
        .add_service(tonic_web::enable(event_grpc))
        .add_service(tonic_web::enable(registration_schema_grpc))
        .add_service(tonic_web::enable(registration_grpc))
        .add_service(tonic_web::enable(organization_grpc))
        .add_service(tonic_web::enable(user_grpc))
        .add_service(tonic_web::enable(authentication_web_grpc))
        .add_service(tonic_web::enable(permission_grpc))
        .serve(grpc_web_addr);

    let server_state = to_state(InternalServer::new(internal_server));

    let dioxus_config = ServeConfig::builder()
        .context_providers(Arc::new(vec![server_state]))
        .build()?;

    let webserver =
        axum::Router::new().serve_dioxus_application(dioxus_config, crate::view::app::App);
    let addr = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let axum_server = axum::serve(listener, webserver);

    grpc_server.await.map_err(ServerError::GrpcError)?;
    grpc_web_server.await.map_err(ServerError::GrpcError)?;
    axum_server.await?;

    Ok(())
}

fn to_state<T>(anything: T) -> Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync + 'static>
where
    T: std::any::Any + Send + Sync + 'static + Clone,
{
    Box::new(move || Box::new(anything.clone()) as Box<dyn std::any::Any>)
        as Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync + 'static>
}

#[derive(Error, Debug)]
enum ServerError {
    #[error("failed to start grpc server: {0}")]
    GrpcError(transport::Error),
}

fn service_name<T: NamedService>(_service: &T) -> &'static str {
    T::NAME
}
