#![cfg(feature = "server")]

use crate::{
    api::{
        authentication::Service as AuthenticationService, event::Service as EventService,
        organization::Service as OrganizationService, registration::Service as RegistrationService,
        registration_schema::Service as SchemaService, user::Service as UserService,
    },
    server_functions::{
        event::AnyService as AnyEventService, organization::AnyService as AnyOrganizationService,
        registration_schema::AnyService as AnyRegistrationSchemaService,
    },
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

    let event_service = Arc::new(EventService::new(event_store));

    let schema_service = Arc::new(SchemaService::new(schema_store));

    let registration_service = Arc::new(RegistrationService::new(registration_store));

    let organization_service = Arc::new(OrganizationService::new(organization_store));

    let authentication_service =
        Arc::new(AuthenticationService::new(key_manager, user_store.clone()));

    let user_service = Arc::new(UserService::new(user_store));

    let event_grpc =
        proto::event_service_server::EventServiceServer::from_arc(event_service.clone());

    let registration_schema_grpc =
        proto::registration_schema_service_server::RegistrationSchemaServiceServer::from_arc(
            schema_service.clone(),
        );

    let registration_grpc = proto::registration_service_server::RegistrationServiceServer::from_arc(
        registration_service.clone(),
    );

    let organization_grpc = proto::organization_service_server::OrganizationServiceServer::from_arc(
        organization_service.clone(),
    );

    let user_grpc = proto::user_service_server::UserServiceServer::from_arc(user_service.clone());

    let authentication_grpc =
        proto::authentication_service_server::AuthenticationServiceServer::from_arc(
            authentication_service.clone(),
        );

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let grpc_addr = "[::1]:50050".parse()?;

    let grpc_server = Server::builder()
        .add_service(event_grpc.clone())
        .add_service(registration_schema_grpc.clone())
        .add_service(registration_grpc.clone())
        .add_service(organization_grpc.clone())
        .add_service(user_grpc.clone())
        .add_service(authentication_grpc.clone())
        .add_service(reflection_service)
        .serve(grpc_addr);

    let grpc_web_addr = "[::1]:50051".parse()?;

    let grpc_web_server = Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(event_grpc))
        .add_service(tonic_web::enable(registration_schema_grpc))
        .add_service(tonic_web::enable(registration_grpc))
        .add_service(tonic_web::enable(organization_grpc))
        .add_service(tonic_web::enable(user_grpc))
        .add_service(tonic_web::enable(authentication_grpc))
        .serve(grpc_web_addr);

    let organization_provider_state = Box::new(move || {
        Box::new(AnyOrganizationService::new_sqlite(
            organization_service.clone(),
        )) as Box<dyn std::any::Any>
    })
        as Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync + 'static>;

    let event_provider_state = Box::new(move || {
        Box::new(AnyEventService::new_sqlite(event_service.clone())) as Box<dyn std::any::Any>
    })
        as Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync + 'static>;

    let registration_schema_provider_state = Box::new(move || {
        Box::new(AnyRegistrationSchemaService::new_sqlite(
            schema_service.clone(),
        )) as Box<dyn std::any::Any>
    })
        as Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync + 'static>;

    let dioxus_config = ServeConfig::builder()
        .context_providers(Arc::new(vec![
            event_provider_state,
            organization_provider_state,
            registration_schema_provider_state,
        ]))
        .build()?;

    let webserver =
        axum::Router::new().serve_dioxus_application(dioxus_config, crate::view::app::App);
    let addr = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let server = axum::serve(listener, webserver);

    let (grpc_err, grpc_web_err, axum_err) =
        futures::join!(grpc_server, grpc_web_server, server.into_future());
    grpc_err.map_err(ServerError::GrpcError)?;
    grpc_web_err.map_err(ServerError::GrpcError)?;
    axum_err?;

    Ok(())
}

#[derive(Error, Debug)]
enum ServerError {
    #[error("failed to start grpc server: {0}")]
    GrpcError(transport::Error),
}
