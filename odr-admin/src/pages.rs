pub mod event;
pub mod events;
pub mod not_found;
pub mod organizations;
pub mod registration;
pub mod registration_schema;

use dioxus::prelude::*;
use dioxus_router::prelude::*;
use event::Page as EventPage;
use events::Page as EventsPage;
use not_found::Page as NotFound;
use organizations::Page as OrganizationsPage;
use registration::Page as RegistrationPage;
use registration_schema::Page as RegistrationSchemaPage;

#[derive(Routable, Clone)]
pub enum Routes {
    #[route("/")]
    OrganizationsPage,

    #[route("/organizations/:org_id")]
    EventsPage { org_id: String },

    #[route("/events/:id")]
    EventPage { id: String },

    #[route("/events/:id/registration_schema")]
    RegistrationSchemaPage { id: String },

    #[route("/events/:event_id/registrations")]
    RegistrationPage { event_id: String },

    #[route("/404")]
    NotFound,
}
