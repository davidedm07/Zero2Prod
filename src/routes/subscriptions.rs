use crate::{
    database_helper::{
        get_subscriber_id_from_email, get_subscription_token_from_id, insert_subscriber,
        store_token,
    },
    domain::{Subscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
};
use actix_web::{web, HttpResponse};

use rand::{distributions::Alphanumeric, thread_rng, Rng};
use reqwest::Url;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
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
) -> HttpResponse {
    let subscriber: Subscriber = match form_data.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if let Some(subscriber_id) =
        get_subscriber_id_from_email(&db_connection_pool, &subscriber.email.as_ref())
            .await
            .unwrap()
    {
        let subscription_token =
            match get_subscription_token_from_id(&db_connection_pool, subscriber_id)
                .await
                .unwrap()
            {
                Some(token) => token,
                None => return HttpResponse::InternalServerError().finish(),
            };

        if send_confirmation_email(&email_client, subscriber, &base_url, &subscription_token)
            .await
            .is_err()
        {
            tracing::error!("Failed to send confirmation email");
            return HttpResponse::InternalServerError().finish();
        }
        return HttpResponse::Ok().finish();
    }

    let mut transaction = match db_connection_pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscriber_id = match insert_subscriber(&subscriber, &mut transaction).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => {
            tracing::error!("Failed to insert subscriber");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let subscription_token = generate_subscription_token();

    if store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .is_err()
    {
        tracing::error!("Failed to store subscription token into database");
        return HttpResponse::InternalServerError().finish();
    }

    if transaction.commit().await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    if send_confirmation_email(&email_client, subscriber, &base_url, &subscription_token)
        .await
        .is_err()
    {
        tracing::error!("Failed to send confirmation email");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
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
        .send_email(subscriber.email, "Welcome", &plain_body, &html_body)
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
