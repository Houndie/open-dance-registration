use dioxus::prelude::*;

use crate::view::pages::landing::Page as LandingPage;

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
