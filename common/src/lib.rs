pub mod proto {
    tonic::include_proto!("proto");
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("descriptors");
}

pub mod rest {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub enum LoginRequest {
        Credentials { email: String, password: String },
        Cookie,
    }

    #[derive(Deserialize, Serialize)]
    pub struct LoginResponse {
        pub token: String,
    }
}
