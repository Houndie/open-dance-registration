use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("descriptors.bin"))
        .compile_protos(
            &[
                "proto/authentication.proto",
                "proto/event.proto",
                "proto/organization.proto",
                "proto/permission.proto",
                "proto/queries.proto",
                "proto/registration_schema.proto",
                "proto/registration.proto",
                "proto/user.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
