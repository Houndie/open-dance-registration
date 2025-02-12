fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "web")]
    // Hydrate the application on the client
    dioxus::launch(odr_server::view::app::App);

    #[cfg(feature = "server")]
    {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                odr_server::server::run_server().await.unwrap();
            });
    }
    Ok(())
}
