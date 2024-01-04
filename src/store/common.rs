use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::now_v7()
        .hyphenated()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_owned()
}
