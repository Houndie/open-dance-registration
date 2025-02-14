use dioxus::prelude::*;

use crate::{
    server_functions,
    view::pages::{
        event::Page as EventPage, landing::Page as LandingPage, login::Page as LoginPage,
        not_found::Page as NotFound, organization::Page as OrganizationPage,
        profile::Page as ProfilePage, registration::Page as RegistrationPage,
        registration_schema::Page as RegistrationSchemaPage,
        server_settings::Page as ServerSettings,
    },
};

#[derive(Clone, Debug, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum Error {
    #[error("Not found")]
    NotFound,
    #[error("{0}")]
    Misc(String),
    #[error("Server function error: {0}")]
    ServerFunctionError(String),
    #[error("unauthenticated")]
    Unauthenticated,
}

impl Error {
    pub fn from_server_fn_error(e: server_functions::Error) -> Self {
        if let server_functions::Error::GrpcError(e) = &e {
            if e.code() == tonic::Code::Unauthenticated {
                return Self::Unauthenticated;
            }
        }

        Self::ServerFunctionError(e.to_string())
    }
}

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

    #[route("/events/:id/registration_schema")]
    RegistrationSchemaPage { id: String },

    #[route("/events/:id/registrations")]
    RegistrationPage { id: String },

    #[route("/login")]
    LoginPage,

    #[route("/profile")]
    ProfilePage,

    #[route("/404")]
    NotFound,

    #[route("/settings")]
    ServerSettings,
}
