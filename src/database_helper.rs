use std::{error::Error, fmt::Debug};

use crate::{
    domain::{Subscriber, SubscriberEmail},
    telemetry::error_chain_fmt,
};
use anyhow::Context;
use chrono::Utc;
use secrecy::Secret;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subcription token"
        )
    }
}

impl Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct RetrieveTokenError(sqlx::Error);

impl std::fmt::Display for RetrieveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to retrieve the subcription token from the database"
        )
    }
}

impl Error for RetrieveTokenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl Debug for RetrieveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct RetrieveSubscriberError(sqlx::Error);

impl std::fmt::Display for RetrieveSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to retrieve the subcriber/s from the database"
        )
    }
}

impl Error for RetrieveSubscriberError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl Debug for RetrieveSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[tracing::instrument(name = "Saving subscriber in database", skip(subscriber, transaction))]
pub async fn insert_subscriber(
    subscriber: &Subscriber,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at, status) VALUES ($1, $2, $3, $4, 'pending confirmation')"#,
        subscriber_id,
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(transaction)
    .await?;

    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Storing subscription token into database",
    skip(transaction, subscription_token)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id,
    )
    .execute(transaction)
    .await
    .map_err(|e| StoreTokenError(e))?;

    Ok(())
}

#[tracing::instrument(
    name = "Retrieving subscription_id from subscription_token",
    skip(subscription_token, db_connection_pool)
)]
pub async fn get_subscriber_id_from_token(
    db_connection_pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, RetrieveSubscriberError> {
    let result = sqlx::query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token=$1"#,
        subscription_token
    )
    .fetch_optional(db_connection_pool)
    .await
    .map_err(|e| RetrieveSubscriberError(e))?;
    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(
    name = "Mark subscription as confirmed",
    skip(subscriber_id, db_connection_pool)
)]
pub async fn confirm_subscriber(
    db_connection_pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status='confirmed' WHERE id=$1"#,
        subscriber_id
    )
    .execute(db_connection_pool)
    .await?;
    Ok(())
}

#[tracing::instrument(
    name = "Retrieve subscriber id from email",
    skip(subscriber_email, db_connection_pool)
)]
pub async fn get_subscriber_id_from_email(
    db_connection_pool: &PgPool,
    subscriber_email: &str,
) -> Result<Option<Uuid>, RetrieveSubscriberError> {
    let record = sqlx::query!(
        r#"SELECT id FROM subscriptions WHERE email=$1"#,
        subscriber_email
    )
    .fetch_optional(db_connection_pool)
    .await
    .map_err(|e| RetrieveSubscriberError(e))?;
    Ok(record.map(|r| r.id))
}

pub async fn get_subscription_token_from_id(
    db_connection_pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<Option<String>, RetrieveTokenError> {
    let record = sqlx::query!(
        r#"SELECT subscription_token FROM subscription_tokens WHERE subscriber_id=$1"#,
        subscriber_id
    )
    .fetch_optional(db_connection_pool)
    .await
    .map_err(|e| RetrieveTokenError(e))?;

    Ok(record.map(|r| r.subscription_token))
}

pub struct ConfirmedSubscriber {
    pub email: SubscriberEmail,
}

pub async fn get_confirmed_subscribers(
    db_connection_pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, RetrieveSubscriberError> {
    let rows = sqlx::query!(r#"SELECT email FROM subscriptions WHERE status='confirmed'"#,)
        .fetch_all(db_connection_pool)
        .await
        .map_err(|e| RetrieveSubscriberError(e))?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|row| match SubscriberEmail::parse(row.email) {
            Ok(subscriber_email) => Ok(ConfirmedSubscriber {
                email: subscriber_email,
            }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}

#[tracing::instrument(name = "Get stored credentials", skip(db_connection_pool, username))]
pub async fn get_stored_credentials(
    username: &str,
    db_connection_pool: &PgPool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"SELECT user_id, password FROM users WHERE username = $1"#,
        username
    )
    .fetch_optional(db_connection_pool)
    .await
    .context("Failed to retrieve stored credentials")?
    .map(|row| (row.user_id, Secret::new(row.password)));

    Ok(row)
}
