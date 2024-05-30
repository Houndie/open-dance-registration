use common::proto::Claims;
use dioxus::prelude::*;

#[derive(Debug, Clone)]
pub enum LoginState {
    LoggedIn(Claims),
    LoggedOut,
    Unknown,
}

pub fn use_login_provider() {
    use_context_provider(|| Signal::new(LoginState::Unknown));
}

pub fn use_login() -> Signal<LoginState> {
    use_context::<Signal<LoginState>>()
}
