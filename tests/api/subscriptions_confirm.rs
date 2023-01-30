use reqwest::Response;

use crate::helpers::spawn_app;

#[tokio::test]
async fn confirmation_without_token_are_rejected_with_a_400() {
    let test_app = spawn_app().await;

    let response = reqwest::get(format!("{}/subscriptions/confirm", test_app.address))
        .await
        .unwrap();

    assert_eq!(400, response.status().as_u16());
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    test_app.email_mock_200_response().await;
    test_app.post_subscriptions(body.into()).await;

    let response: Response = test_app.call_confirmation_link().await;
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn clicking_confirmation_link_set_status_to_confirmed_in_db() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    test_app.email_mock_200_response().await;
    test_app.post_subscriptions(body.into()).await;
    test_app.call_confirmation_link().await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&test_app.db_connection_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "jondoe@email.com");
    assert_eq!(saved.name, "Jon Doe");
    assert_eq!(saved.status, "confirmed");
}
