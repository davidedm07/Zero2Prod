use crate::helpers::spawn_app;

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    test_app.email_mock_200_response().await;
    let response = test_app.post_subscriptions(body.into()).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_returns_400_when_data_is_missing_or_empty() {
    let test_app = spawn_app().await;

    let test_cases = vec![
        ("", "missing email and name"),
        ("name=Jon", "missing email"),
        ("email=jon%40email.com", "missing name"),
        ("name=&email=jon%40email.com", "empty name"),
        ("name=Jon&email=", "empty email"),
        ("name=Jon&email=not-an-email", "invalid email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = test_app.post_subscriptions(invalid_body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "API did not fail with 400 bad request when payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_sends_confirmation_email() {
    let test_app = spawn_app().await;

    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    test_app.email_mock_200_response().await;

    let response = test_app.post_subscriptions(body.into()).await;
    let email_request = &test_app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = test_app.get_confirmation_links(email_request);

    let html_link = confirmation_links.html;
    let text_link = confirmation_links.plain_text;

    assert_eq!(html_link, text_link);
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_persists_subscriber() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    test_app.email_mock_200_response().await;
    test_app.post_subscriptions(body.into()).await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&test_app.db_connection_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "jondoe@email.com");
    assert_eq!(saved.name, "Jon Doe");
    assert_eq!(saved.status, "pending confirmation");
}

#[tokio::test]
async fn subcribing_twice_sends_two_confirmation_emails() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    test_app.email_mock_200_response_with_times(2).await;
    test_app.post_subscriptions(body.into()).await;
    let second_request = test_app.post_subscriptions(body.into()).await;

    let email_request = &test_app.email_server.received_requests().await.unwrap();

    assert_eq!(2, email_request.len());
    assert_eq!(200, second_request.status().as_u16());
}
