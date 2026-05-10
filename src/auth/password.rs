use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("failed to hash password")]
    HashError,
    #[error("invalid password")]
    VerifyError,
}

/// A pre-computed argon2 hash used as a dummy when the user is not found.
/// Running verify_password against this eliminates timing differences between
/// "user not found" and "wrong password" paths.
pub const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$YXNkZmFzZGZhc2RmYXNk$N0K0ZWWJF4L5KLpKbGBqMbsV8pQ0YtMa5M8+Lm4Fk3I";

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| PasswordError::HashError)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<(), PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(|_| PasswordError::VerifyError)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| PasswordError::VerifyError)
}

/// Hash a page-sharing password using argon2.
/// Limits input to 128 chars to prevent DoS.
pub fn hash_page_password(password: &str) -> Result<String, PasswordError> {
    if password.len() > 128 {
        return Err(PasswordError::HashError);
    }
    hash_password(password)
}

/// Verify a page password against stored value.
/// Supports both argon2 hashes (new) and legacy plaintext (backward compat).
/// Uses constant-time comparison for both paths.
pub fn verify_page_password(password: &str, stored: &str) -> Result<(), PasswordError> {
    if stored.starts_with("$argon2") {
        verify_password(password, stored)
    } else {
        // Legacy plaintext: constant-time comparison
        use subtle::ConstantTimeEq;
        if bool::from(password.as_bytes().ct_eq(stored.as_bytes())) {
            Ok(())
        } else {
            Err(PasswordError::VerifyError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_password() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(hash.starts_with("$argon2"));
        verify_password(password, &hash).unwrap();
    }

    #[test]
    fn wrong_password_fails() {
        let hash = hash_password("correct").unwrap();
        let result = verify_password("wrong", &hash);
        assert!(result.is_err());
    }

    #[test]
    fn different_hashes_for_same_password() {
        let h1 = hash_password("same").unwrap();
        let h2 = hash_password("same").unwrap();
        assert_ne!(h1, h2, "salts should differ");
    }

    #[test]
    fn hash_and_verify_page_password() {
        let hash = hash_page_password("my-secret").unwrap();
        assert!(hash.starts_with("$argon2"));
        verify_page_password("my-secret", &hash).unwrap();
    }

    #[test]
    fn wrong_page_password_fails() {
        let hash = hash_page_password("correct").unwrap();
        assert!(verify_page_password("wrong", &hash).is_err());
    }

    #[test]
    fn page_password_max_length() {
        let long = "a".repeat(129);
        assert!(hash_page_password(&long).is_err());
    }

    #[test]
    fn verify_legacy_plaintext() {
        // Legacy plaintext stored passwords should still work
        assert!(verify_page_password("mypassword", "mypassword").is_ok());
        assert!(verify_page_password("wrong", "mypassword").is_err());
    }
}
