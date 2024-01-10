pub mod event;
pub mod events;
pub mod registration_schema;

use dioxus::prelude::*;
use dioxus_router::prelude::*;
use event::Page as EventPage;
use events::Page as EventsPage;
use registration_schema::Page as RegistrationSchemaPage;

#[derive(Routable, Clone)]
pub enum Routes {
    #[route("/")]
    EventsPage {},

    #[route("/events/:id")]
    EventPage { id: String },

    #[route("/events/:id/registration_schema")]
    RegistrationSchemaPage { id: String },
}
