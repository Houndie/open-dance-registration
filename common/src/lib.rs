pub mod proto {
    tonic::include_proto!("event");
    tonic::include_proto!("registration_schema");
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("descriptors");
}
