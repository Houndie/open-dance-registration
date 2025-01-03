#![cfg(feature = "server")]

use crate::{
    api::{
        authentication::Service as AuthenticationService, event::Service as EventService,
        organization::Service as OrganizationService, registration::Service as RegistrationService,
        registration_schema::Service as SchemaService, user::Service as UserService,
    },
    server_functions::organization::AnyService as AnyOrganizationService,
};
use common::proto;
use dioxus::prelude::{DioxusRouterExt, ServeConfig};
use odr_core::store::{
    event::SqliteStore as EventStore, keys::SqliteStore as KeyStore,
    organization::SqliteStore as OrganizationStore, registration::SqliteStore as RegistrationStore,
    registration_schema::SqliteStore as SchemaStore, user::SqliteStore as UserStore,
};
use sqlx::SqlitePool;
use std::{env, future::IntoFuture, sync::Arc};
use thiserror::Error;
use tonic::transport::{self, Server};

fn db_url() -> String {
    format!("sqlite://{}/odr-sqlite.db", env::temp_dir().display())
}

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = db_url();

    let db = Arc::new(SqlitePool::connect(&db_url).await?);

    let event_store = Arc::new(EventStore::new(db.clone()));
    let schema_store = Arc::new(SchemaStore::new(db.clone()));
    let registration_store = Arc::new(RegistrationStore::new(db.clone()));
    let organization_store = Arc::new(OrganizationStore::new(db.clone()));
    let user_store = Arc::new(UserStore::new(db.clone()));
    let key_store = Arc::new(KeyStore::new(db.clone()));

    let key_manager = Arc::new(odr_core::keys::KeyManager::new(key_store));

    let event_service =
        proto::event_service_server::EventServiceServer::new(EventService::new(event_store));

    let schema_service =
        proto::registration_schema_service_server::RegistrationSchemaServiceServer::new(
            SchemaService::new(schema_store),
        );

    let registration_service = proto::registration_service_server::RegistrationServiceServer::new(
        RegistrationService::new(registration_store),
    );

    let organization_service = Arc::new(OrganizationService::new(organization_store));

    let authentication_service =
        proto::authentication_service_server::AuthenticationServiceServer::new(
            AuthenticationService::new(key_manager, user_store.clone()),
        );

    let user_service =
        proto::user_service_server::UserServiceServer::new(UserService::new(user_store));

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    let grpc_addr = "[::1]:50051".parse()?;

    let grpc_server = Server::builder()
        .accept_http1(true)
        .add_service(event_service)
        .add_service(schema_service)
        .add_service(registration_service)
        .add_service(
            proto::organization_service_server::OrganizationServiceServer::from_arc(
                organization_service.clone(),
            ),
        )
        .add_service(user_service)
        .add_service(authentication_service)
        .add_service(reflection_service)
        .serve(grpc_addr);

    let organization_provider_state = Box::new(move || {
        Box::new(AnyOrganizationService::new_sqlite(
            organization_service.clone(),
        )) as Box<dyn std::any::Any>
    })
        as Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync + 'static>;

    let dioxus_config = ServeConfig::builder()
        .context_providers(Arc::new(vec![organization_provider_state]))
        .build()?;
    let webserver =
        axum::Router::new().serve_dioxus_application(dioxus_config, crate::view::app::App);
    let addr = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let server = axum::serve(listener, webserver);
    let (grpc_err, axum_err) = futures::join!(grpc_server, server.into_future());
    grpc_err.map_err(ServerError::GrpcError)?;
    axum_err?;

    Ok(())
}

#[derive(Error, Debug)]
enum ServerError {
    #[error("failed to start grpc server: {0}")]
    GrpcError(transport::Error),
}
