pub mod event;
pub mod events;
pub mod not_found;
pub mod organizations;
pub mod profile;
pub mod registration;
pub mod registration_schema;

use dioxus::prelude::*;
use event::Page as EventPage;
use events::Page as EventsPage;
use not_found::Page as NotFound;
use organizations::Page as OrganizationsPage;
use profile::Page as ProfilePage;
use registration::Page as RegistrationPage;
use registration_schema::Page as RegistrationSchemaPage;

#[derive(Clone, PartialEq, Routable)]
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

    #[route("/profile")]
    ProfilePage,

    #[route("/404")]
    NotFound,
}
