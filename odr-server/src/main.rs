use std::{env, sync::Arc};

use api::{event::Service as EventService, registration_schema::Service as SchemaService};
use common::proto;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use store::{event::SqliteStore as EventStore, registration_schema::SqliteStore as SchemaStore};
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

    let event_service =
        proto::event_service_server::EventServiceServer::new(EventService::new(event_store));

    let schema_service =
        proto::registration_schema_service_server::RegistrationSchemaServiceServer::new(
            SchemaService::new(schema_store),
        );

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(event_service))
        .add_service(tonic_web::enable(schema_service))
        .add_service(tonic_web::enable(reflection_service))
        .serve("[::1]:50051".parse()?)
        .await?;

    Ok(())
}
