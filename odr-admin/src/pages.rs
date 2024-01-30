pub mod event;
pub mod events;
pub mod organizations;
pub mod registration_schema;

use dioxus::prelude::*;
use dioxus_router::prelude::*;
use event::Page as EventPage;
use events::Page as EventsPage;
use organizations::Page as OrganizationsPage;
use registration_schema::Page as RegistrationSchemaPage;

#[derive(Routable, Clone)]
pub enum Routes {
    #[route("/")]
    OrganizationsPage,

    #[route("/events")]
    EventsPage { org_id: String },

    #[route("/events/:id")]
    EventPage { id: String },

    #[route("/events/:id/registration_schema")]
    RegistrationSchemaPage { id: String },
}
