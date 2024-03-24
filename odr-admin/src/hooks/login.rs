use dioxus::prelude::*;

pub struct Login(pub bool);

pub fn use_login_provider(cx: Scope) {
    use_shared_state_provider(cx, || Login(false));
}

pub fn use_login(cx: Scope) -> Option<&UseSharedState<Login>> {
    use_shared_state::<Login>(cx)
}
