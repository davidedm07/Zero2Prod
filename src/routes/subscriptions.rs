use crate::{
    domain::{Subscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
};
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
    skip(form_data, db_connection_pool, email_client),
    fields(
        subscriber_email = %form_data.email,
        subscriber_name = %form_data.name
    )
)]
pub async fn subscribe(
    form_data: web::Form<FormData>,
    db_connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> HttpResponse {
    let subscriber = match form_data.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if insert_subscriber(&subscriber, &db_connection_pool)
        .await
        .is_err()
    {
        tracing::error!("Failed to insert subscriber");
        return HttpResponse::InternalServerError().finish();
    }

    if email_client
        .send_email(
            subscriber.email,
            "Welcome",
            "Welcome to the newsletter",
            "Welcome to the newsletter",
        )
        .await
        .is_err()
    {
        tracing::error!("Failed to send confirmation email");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
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
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at, status) VALUES ($1, $2, $3, $4, 'confirmed')"#,
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

impl TryFrom<FormData> for Subscriber {
    type Error = String;

    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;

        Ok(Self { name, email })
    }
}
