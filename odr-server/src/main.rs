use dioxus::prelude::*;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "server")]
pub mod api;

pub mod server_functions;
pub mod view;

use view::app::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "web")]
    // Hydrate the application on the client
    dioxus::launch(App);

    #[cfg(feature = "server")]
    {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                server::run_server().await.unwrap();
            });
    }
    Ok(())
}
