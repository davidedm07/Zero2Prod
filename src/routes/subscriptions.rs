use crate::domain::{Subscriber, SubscriberEmail, SubscriberName};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form_data, db_connection_pool),
    fields(
        subscriber_email = %form_data.email,
        subscriber_name = %form_data.name
    )
)]
pub async fn subscribe(
    form_data: web::Form<FormData>,
    db_connection_pool: web::Data<PgPool>,
) -> HttpResponse {
    let subscriber_name = match SubscriberName::parse(form_data.0.name) {
        Ok(subscriber_name) => subscriber_name,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let subscriber_email = match SubscriberEmail::parse(form_data.0.email) {
        Ok(subscriber_email) => subscriber_email,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let subscriber = Subscriber {
        name: subscriber_name,
        email: subscriber_email,
    };

    match insert_subscriber(&subscriber, &db_connection_pool).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(
    name = "Saving subscriber in database",
    skip(subscriber, db_connection_pool)
)]
pub async fn insert_subscriber(
    subscriber: &Subscriber,
    db_connection_pool: &web::Data<PgPool>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)"#,
        Uuid::new_v4(),
        subscriber.name.as_ref(),
        subscriber.email.as_ref(),
        Utc::now()
    )
    .execute(db_connection_pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(())
}
