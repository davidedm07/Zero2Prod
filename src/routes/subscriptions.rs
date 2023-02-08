use crate::{
    database_helper::{
        get_subscriber_id_from_email, get_subscription_token_from_id, insert_subscriber,
        store_token,
    },
    domain::{Subscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    telemetry::error_chain_fmt,
};
use actix_web::{web, HttpResponse, ResponseError};

use anyhow::Context;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("${0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form_data, db_connection_pool, email_client, base_url),
    fields(
        subscriber_email = %form_data.email,
        subscriber_name = %form_data.name
    )
)]
pub async fn subscribe(
    form_data: web::Form<FormData>,
    db_connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<Url>,
) -> Result<HttpResponse, SubscribeError> {
    let subscriber: Subscriber = match form_data.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return Ok(HttpResponse::BadRequest().finish()),
    };

    let confirmation_email_error_message = "Failed to send confirmation email";

    if let Some(subscriber_id) =
        get_subscriber_id_from_email(&db_connection_pool, &subscriber.email.as_ref())
            .await
            .context("Failed to get the subscriber from input email")?
    {
        let subscription_token =
            match get_subscription_token_from_id(&db_connection_pool, subscriber_id)
                .await
                .context("Failed to get the subcription token from subscriber id")?
            {
                Some(token) => token,
                None => return Ok(HttpResponse::InternalServerError().finish()),
            };

        send_confirmation_email(&email_client, subscriber, &base_url, &subscription_token)
            .await
            .context(confirmation_email_error_message)?;

        return Ok(HttpResponse::Ok().finish());
    };

    let mut transaction = db_connection_pool
        .begin()
        .await
        .context("Failed to get the connection pool while beginning the transaction")?;

    let subscriber_id = insert_subscriber(&subscriber, &mut transaction)
        .await
        .context("Failed to insert the subcriber into the database")?;
    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the subscription token")?;

    transaction
        .commit()
        .await
        .context("Failed to commit the transaction")?;

    send_confirmation_email(&email_client, subscriber, &base_url, &subscription_token)
        .await
        .context(confirmation_email_error_message)?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Sending confirmation email",
    skip(email_client, subscriber, base_url)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    subscriber: Subscriber,
    base_url: &Url,
    confirmation_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = base_url
        .join(
            format!(
                "/subscriptions/confirm?subscription_token={}",
                confirmation_token
            )
            .as_str(),
        )
        .unwrap();

    let plain_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm subscription",
        confirmation_link
    );

    let html_body = format!(
        "Welcome to our newsletter!<br />\
    Click <a href=\"{}\"> here </a> to confirm your subscription.",
        confirmation_link
    );

    email_client
        .send_email(&subscriber.email, "Welcome", &plain_body, &html_body)
        .await
}

impl TryFrom<FormData> for Subscriber {
    type Error = String;

    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;

        Ok(Self { name, email })
    }
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}
