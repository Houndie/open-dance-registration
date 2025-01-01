#![cfg(feature = "server")]

use argon2::{
    password_hash::{PasswordHashString, SaltString},
    Argon2, PasswordHasher as _,
};
use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<PasswordHashString, argon2::password_hash::Error> {
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))?
        .serialize())
}
