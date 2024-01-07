use std::{env, sync::Arc};

use api::event::EventService;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use store::event::SqliteStore;
use tonic::transport::Server;

pub mod api;
pub mod store;

mod proto {
    tonic::include_proto!("event");
    tonic::include_proto!("registration_schema");
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("descriptors");
}

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

    let event_store = Arc::new(SqliteStore::new(db));

    let event_service =
        proto::event_service_server::EventServiceServer::new(EventService::new(event_store));

    let event_reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    Server::builder()
        .add_service(event_service)
        .add_service(event_reflection_service)
        .serve("[::1]:50051".parse()?)
        .await?;

    Ok(())
}
