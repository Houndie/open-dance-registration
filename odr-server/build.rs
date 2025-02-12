use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("descriptors.bin"))
        .compile(
            &[
                "../proto/event.proto",
                "../proto/registration_schema.proto",
                "../proto/registration.proto",
                "../proto/organization.proto",
                "../proto/user.proto",
                "../proto/queries.proto",
                "../proto/authentication.proto",
            ],
            &["../proto"],
        )?;
    Ok(())
}
