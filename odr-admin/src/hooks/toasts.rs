use dioxus::prelude::*;

#[derive(Default)]
pub struct ToastManager {
    toasts: Vec<Toast>,
}

pub struct Toast {
    pub title: String,
    pub body: String,
}

impl ToastManager {
    pub fn new_error(&mut self, error_message: String) {
        log::error!("Error occurred while fetching events: {}", error_message);
        self.toasts.push(Toast {
            title: "Oh no!".to_owned(),
            body: "We're sorry, something unexpected went wrong".to_owned(),
        })
    }

    pub fn toasts(&self) -> std::slice::Iter<'_, Toast> {
        self.toasts.iter()
    }

    pub fn remove_toast(&mut self, idx: usize) {
        self.toasts.remove(idx);
    }
}

pub struct ToastsContext(pub ToastManager);

pub fn use_toasts_provider(cx: &ScopeState) {
    use_shared_state_provider(cx, || ToastsContext(ToastManager::default()))
}

pub fn use_toasts(cx: &ScopeState) -> Option<&UseSharedState<ToastsContext>> {
    use_shared_state::<ToastsContext>(cx)
}
