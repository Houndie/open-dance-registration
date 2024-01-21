use std::{env, sync::Arc};

use api::{
    event::Service as EventService, organization::Service as OrganizationService,
    registration::Service as RegistrationService, registration_schema::Service as SchemaService,
    user::Service as UserService,
};
use common::proto;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use store::{
    event::SqliteStore as EventStore, organization::SqliteStore as OrganizationStore,
    registration::SqliteStore as RegistrationStore,
    registration_schema::SqliteStore as SchemaStore, user::SqliteStore as UserStore,
};
use tonic::transport::Server;

pub mod api;
pub mod store;

fn db_url() -> String {
    format!("sqlite://{}/odr-sqlite.db", env::temp_dir().display())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = db_url();
    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        Sqlite::create_database(&db_url).await?;
    }

    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    sqlx::migrate!().run(&(*db)).await?;

    let event_store = Arc::new(EventStore::new(db.clone()));
    let schema_store = Arc::new(SchemaStore::new(db.clone()));
    let registration_store = Arc::new(RegistrationStore::new(db.clone()));
    let organization_store = Arc::new(OrganizationStore::new(db.clone()));
    let user_store = Arc::new(UserStore::new(db.clone()));

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
        proto::user_service_server::UserServiceServer::new(UserService::new(user_store));

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(event_service))
        .add_service(tonic_web::enable(schema_service))
        .add_service(tonic_web::enable(registration_service))
        .add_service(tonic_web::enable(organization_service))
        .add_service(tonic_web::enable(user_service))
        .add_service(tonic_web::enable(reflection_service))
        .serve("[::1]:50051".parse()?)
        .await?;

    Ok(())
}
