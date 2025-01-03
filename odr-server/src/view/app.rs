use dioxus::prelude::*;

use crate::view::organization::Page as OrganizationsPage;

#[component]
pub fn App() -> Element {
    rsx! { Router::<Routes>{} }
}

#[derive(Clone, PartialEq, Routable)]
pub enum Routes {
    #[route("/")]
    OrganizationsPage,
    /*#[route("/organizations/:org_id")]
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
    NotFound,*/
}
