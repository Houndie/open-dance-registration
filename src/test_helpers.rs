use tonic::{metadata::KeyAndValueRef, Status};

#[derive(Debug)]
pub struct StatusCompare {
    status: Status,
}

impl StatusCompare {
    pub fn new(status: Status) -> Self {
        StatusCompare { status }
    }
}

impl PartialEq for StatusCompare {
    fn eq(&self, other: &Self) -> bool {
        self.status.code() == other.status.code()
            && self.status.message() == other.status.message()
            && self.status.metadata().len() == other.status.metadata().len()
            && self
                .status
                .metadata()
                .iter()
                .zip(other.status.metadata().iter())
                .all(|(a, b)| match (a, b) {
                    (
                        KeyAndValueRef::Ascii(a_key, a_value),
                        KeyAndValueRef::Ascii(b_key, b_value),
                    ) => a_key == b_key && a_value == b_value,
                    (
                        KeyAndValueRef::Binary(a_key, a_value),
                        KeyAndValueRef::Binary(b_key, b_value),
                    ) => a_key == b_key && a_value == b_value,
                    _ => false,
                })
    }
}
