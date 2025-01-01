use dioxus::prelude::*;

#[cfg(feature = "server")]
pub mod grpc;

#[cfg(feature = "server")]
pub mod api;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "web")]
    // Hydrate the application on the client
    dioxus::launch(app);

    #[cfg(feature = "server")]
    {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let webserver =
                    axum::Router::new().serve_dioxus_application(ServeConfig::new().unwrap(), app);
                let addr = dioxus_cli_config::fullstack_address_or_localhost();
                let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
                let server = axum::serve(listener, webserver);
                let grpc_server = crate::grpc::run_grpc();
                server.await.unwrap();
                grpc_server.await.unwrap();
            });
    }
    Ok(())
}

#[component]
fn app() -> Element {
    rsx! {"hello world"}
}
