use crate::view::{
    app::{Error, Routes},
    components::with_toasts::WithToasts,
};
use dioxus::prelude::*;

pub fn use_handle_error<T: Clone, F: FnOnce(T) -> Element>(
    resource: MappedSignal<Result<T, Error>>,
    render: F,
) -> Element {
    let nav = use_navigator();
    let mut redirect_to = use_signal(|| None);
    use_effect(move || {
        if let Some(redirect) = redirect_to.read().clone() {
            nav.push(redirect);
        }
    });

    match resource() {
        Ok(t) => render(t),
        Err(Error::Unauthenticated) => {
            *redirect_to.write() = Some(Routes::LoginPage);
            rsx! {}
        }
        Err(Error::NotFound) => {
            *redirect_to.write() = Some(Routes::NotFound);
            rsx! {}
        }
        Err(e) => rsx! {
            WithToasts{
                initial_errors: vec![e.to_string()],
            }
        },
    }
}
