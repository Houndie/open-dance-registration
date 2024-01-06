use std::{env, sync::Arc};

use proto::{
    DeleteEventsResponse, ListEventsRequest, ListEventsResponse, UpsertEventsRequest,
    UpsertEventsResponse,
};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use store::{
    event::{SqliteStore, Store},
    Error,
};
use tonic::{transport::Server, Code, Request, Response, Status};

pub mod store;

mod proto {
    tonic::include_proto!("event");
    tonic::include_proto!("registration_schema");
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("descriptors");
}

#[derive(Debug)]
struct EventService<EventStoreType: Store> {
    store: Arc<EventStoreType>,
}

impl<EventStoreType: Store> EventService<EventStoreType> {
    fn new(store: Arc<EventStoreType>) -> Self {
        EventService { store }
    }
}

fn store_error_to_status(err: Error) -> Status {
    let code = match err {
        Error::IdDoesNotExist(_) | Error::SomeIdDoesNotExist | Error::UnknownEnum => Code::NotFound,
        Error::InsertionError(_)
        | Error::FetchError(_)
        | Error::UpdateError(_)
        | Error::DeleteError(_)
        | Error::CheckExistsError(_)
        | Error::TransactionStartError(_)
        | Error::TransactionFailed(_)
        | Error::TooManyItems(_)
        | Error::ColumnParseError(_) => Code::Internal,
    };

    Status::new(code, format!("{}", err))
}

#[tonic::async_trait]
impl<EventStoreType: Store> proto::event_service_server::EventService
    for EventService<EventStoreType>
{
    async fn upsert_events(
        &self,
        request: Request<UpsertEventsRequest>,
    ) -> Result<Response<UpsertEventsResponse>, Status> {
        let events = self
            .store
            .upsert(request.into_inner().events)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(UpsertEventsResponse { events }))
    }

    async fn list_events(
        &self,
        request: Request<ListEventsRequest>,
    ) -> Result<Response<ListEventsResponse>, Status> {
        let events = self
            .store
            .list(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;
        Ok(Response::new(ListEventsResponse { events }))
    }

    async fn delete_events(
        &self,
        request: Request<proto::DeleteEventsRequest>,
    ) -> Result<Response<DeleteEventsResponse>, Status> {
        self.store
            .delete(&request.into_inner().ids)
            .await
            .map_err(|e| store_error_to_status(e))?;

        Ok(Response::new(DeleteEventsResponse {}))
    }
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
