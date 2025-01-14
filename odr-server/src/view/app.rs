use dioxus::prelude::*;

use crate::view::pages::{
    event::Page as EventPage, landing::Page as LandingPage, not_found::Page as NotFound,
    organization::Page as OrganizationPage,
};

#[component]
pub fn App() -> Element {
    rsx! {
        Router::<Routes>{}
    }
}

#[derive(Clone, PartialEq, Routable)]
pub enum Routes {
    #[route("/")]
    LandingPage,

    #[route("/organizations/:org_id")]
    OrganizationPage { org_id: String },

    #[route("/events/:id")]
    EventPage { id: String },
    /*#[route("/events/:id/registration_schema")]
    RegistrationSchemaPage { id: String },

    #[route("/events/:event_id/registrations")]
    RegistrationPage { event_id: String },

    #[route("/profile")]
    ProfilePage,*/
    #[route("/404")]
    NotFound,
}
