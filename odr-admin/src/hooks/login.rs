use common::proto::Claims;
use dioxus::prelude::*;

pub struct Login(pub Option<Claims>);

pub fn use_login_provider(cx: Scope) {
    use_shared_state_provider(cx, || Login(None));
}

pub fn use_login(cx: Scope) -> Option<&UseSharedState<Login>> {
    use_shared_state::<Login>(cx)
}
