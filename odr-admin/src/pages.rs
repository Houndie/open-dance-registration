pub mod event;
pub mod events;

use dioxus::prelude::*;
use dioxus_router::prelude::*;
use event::Page as EventPage;
use events::Page as EventsPage;

#[derive(Routable, Clone)]
pub enum Routes {
    #[route("/")]
    EventsPage {},

    #[route("/events/:id")]
    EventPage { id: String },
}
