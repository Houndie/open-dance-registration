use std::{env, io, sync::Arc};

use api::{
    event::Service as EventService, login::api_routes,
    organization::Service as OrganizationService, registration::Service as RegistrationService,
    registration_schema::Service as SchemaService, user::Service as UserService,
};
use axum::http::{
    header::{CONTENT_TYPE, COOKIE},
    HeaderValue,
};
use common::proto;
use sqlx::SqlitePool;
use store::{
    event::SqliteStore as EventStore, keys::SqliteStore as KeyStore,
    organization::SqliteStore as OrganizationStore, registration::SqliteStore as RegistrationStore,
    registration_schema::SqliteStore as SchemaStore, user::SqliteStore as UserStore,
};
use thiserror::Error;
use tokio::{net::TcpListener, task::JoinHandle};
use tonic::transport::{self, Server};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub mod api;
pub mod keys;
pub mod store;
pub mod user;

fn db_url() -> String {
    format!("sqlite://{}/odr-sqlite.db", env::temp_dir().display())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = db_url();

    let db = Arc::new(SqlitePool::connect(&db_url).await?);

    let event_store = Arc::new(EventStore::new(db.clone()));
    let schema_store = Arc::new(SchemaStore::new(db.clone()));
    let registration_store = Arc::new(RegistrationStore::new(db.clone()));
    let organization_store = Arc::new(OrganizationStore::new(db.clone()));
    let user_store = Arc::new(UserStore::new(db.clone()));
    let key_store = Arc::new(KeyStore::new(db.clone()));

    let event_service =
        proto::event_service_server::EventServiceServer::new(EventService::new(event_store));

    let schema_service =
        proto::registration_schema_service_server::RegistrationSchemaServiceServer::new(
            SchemaService::new(schema_store),
        );

    let registration_service = proto::registration_service_server::RegistrationServiceServer::new(
        RegistrationService::new(registration_store),
    );

    let organization_service = proto::organization_service_server::OrganizationServiceServer::new(
        OrganizationService::new(organization_store),
    );

    let user_service =
        proto::user_service_server::UserServiceServer::new(UserService::new(user_store.clone()));

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    let grpc_addr = "[::1]:50051".parse()?;

    let grpc_thread: JoinHandle<Result<(), ServerError>> = tokio::spawn(async move {
        Server::builder()
            .accept_http1(true)
            .add_service(tonic_web::enable(event_service))
            .add_service(tonic_web::enable(schema_service))
            .add_service(tonic_web::enable(registration_service))
            .add_service(tonic_web::enable(organization_service))
            .add_service(tonic_web::enable(user_service))
            .add_service(tonic_web::enable(reflection_service))
            .serve(grpc_addr)
            .await
            .map_err(|e| ServerError::GrpcError(e))?;

        Ok(())
    });

    let login_routes = api_routes(Arc::new(keys::KeyManager::new(key_store)), user_store)
        .layer(
            CorsLayer::new()
                .allow_origin("http://localhost:8080".parse::<HeaderValue>().unwrap())
                .allow_headers([COOKIE, CONTENT_TYPE])
                .allow_credentials(true),
        )
        .layer(TraceLayer::new_for_http());

    let http_listener = TcpListener::bind("0.0.0.0:3000").await?;

    let http_thread: JoinHandle<Result<(), ServerError>> = tokio::spawn(async move {
        axum::serve(http_listener, login_routes)
            .await
            .map_err(|e| ServerError::HttpError(e))?;

        Ok(())
    });

    tokio::try_join!(flatten(grpc_thread), flatten(http_thread))?;

    Ok(())
}

#[derive(Error, Debug)]
enum ServerError {
    #[error("failed to start grpc server: {0}")]
    GrpcError(transport::Error),

    #[error("failed to start http server: {0}")]
    HttpError(io::Error),
}

#[derive(Error, Debug)]
enum FlattenError {
    #[error("failed to join thread: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("error in thread: {0}")]
    Error(ServerError),
}

async fn flatten(handle: JoinHandle<Result<(), ServerError>>) -> Result<(), FlattenError> {
    match handle.await {
        Ok(Ok(t)) => Ok(t),
        Ok(Err(e)) => Err(FlattenError::Error(e)),
        Err(e) => Err(FlattenError::JoinError(e)),
    }
}
