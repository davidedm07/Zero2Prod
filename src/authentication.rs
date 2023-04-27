use anyhow::{anyhow, Context};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

use crate::{database_helper::get_stored_credentials, telemetry::spawn_blocking_with_tracing};

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Authentication Failed")]
    InvalidCredentials(#[source] anyhow::Error),
}

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[tracing::instrument(name = "Validating credentials", skip(db_connection_pool, credentials))]
pub async fn validate_credentials(
    credentials: Credentials,
    db_connection_pool: &PgPool,
) -> Result<uuid::Uuid, AuthError> {
    let (user_id, expected_password) =
        get_stored_credentials(&credentials.username, db_connection_pool)
            .await
            .map_err(AuthError::UnexpectedError)?
            .ok_or_else(|| AuthError::InvalidCredentials(anyhow!("Unknown username.")))?;

    spawn_blocking_with_tracing(move || {
        validate_password_hash(expected_password, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task")
    .map_err(AuthError::UnexpectedError)??;

    Ok(user_id)
}

#[tracing::instrument(
    name = "Validating password hash",
    skip(expected_password, password_candidate)
)]
pub fn validate_password_hash(
    expected_password: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(&expected_password.expose_secret())
        .context("Failed to parse hash in PHC string format")
        .map_err(AuthError::UnexpectedError)?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password")
        .map_err(AuthError::InvalidCredentials)
}
