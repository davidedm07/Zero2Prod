use crate::database_helper::{confirm_subscriber, get_subscriber_id_from_token};
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;
use validator::Validate;

#[derive(Deserialize, Debug, Validate)]
pub struct Parameters {
    #[validate(length(min = 25, max = 25))]
    subscription_token: String,
}

#[tracing::instrument(name = "Confirming a pending subscription", skip(parameters))]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    db_connection_pool: web::Data<PgPool>,
) -> HttpResponse {
    let id = match get_subscriber_id_from_token(&db_connection_pool, &parameters.subscription_token)
        .await
    {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    match id {
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            if confirm_subscriber(&db_connection_pool, subscriber_id)
                .await
                .is_err()
            {
                return HttpResponse::InternalServerError().finish();
            }
            HttpResponse::Ok().finish()
        }
    }
}
