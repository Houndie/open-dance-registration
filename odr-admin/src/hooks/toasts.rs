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
        log::error!("Error occurred: {}", error_message);
        self.toasts.push(Toast {
            title: "Oh no!".to_owned(),
            body: "We're sorry, something unexpected went wrong.".to_owned(),
        })
    }

    pub fn toasts(&self) -> std::slice::Iter<'_, Toast> {
        self.toasts.iter()
    }

    pub fn remove_toast(&mut self, idx: usize) {
        self.toasts.remove(idx);
    }
}

pub fn use_toasts_provider() {
    use_context_provider(|| Signal::new(ToastManager::default()));
}

pub fn use_toasts() -> Signal<ToastManager> {
    use_context::<Signal<ToastManager>>()
}
