use crate::domain::Subscriber;
use chrono::Utc;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id,
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(())
}

#[tracing::instrument(
    name = "Retrieving subscription_id from subscription_token",
    skip(subscription_token, db_connection_pool)
)]
pub async fn get_subscriber_id_from_token(
    db_connection_pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token=$1"#,
        subscription_token
    )
    .fetch_optional(db_connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}

#[tracing::instrument(
    name = "Retrieve subscriber id from email",
    skip(subscriber_email, db_connection_pool)
)]
pub async fn get_subscriber_id_from_email(
    db_connection_pool: &PgPool,
    subscriber_email: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT id FROM subscriptions WHERE email=$1"#,
        subscriber_email
    )
    .fetch_optional(db_connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(record.map(|r| r.id))
}

pub async fn get_subscription_token_from_id(
    db_connection_pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let record = sqlx::query!(
        r#"SELECT subscription_token FROM subscription_tokens WHERE subscriber_id=$1"#,
        subscriber_id
    )
    .fetch_optional(db_connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(record.map(|r| r.subscription_token))
}
