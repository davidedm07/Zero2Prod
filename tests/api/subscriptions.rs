use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, TestApp};

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    email_mock_200_response(&test_app).await;
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

    email_mock_200_response(&test_app).await;

    let response = test_app.post_subscriptions(body.into()).await;
    let email_request = &test_app.email_server.received_requests().await.unwrap()[0];
    let json_body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

    let get_link = |s: &str| {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);
        links[0].as_str().to_owned()
    };

    let html_link = get_link(&json_body["HtmlBody"].as_str().unwrap());
    let text_link = get_link(&json_body["TextBody"].as_str().unwrap());

    assert_eq!(html_link, text_link);
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_persists_subscriber() {
    let test_app = spawn_app().await;
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    email_mock_200_response(&test_app).await;
    test_app.post_subscriptions(body.into()).await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&test_app.db_connection_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "jondoe@email.com");
    assert_eq!(saved.name, "Jon Doe");
    assert_eq!(saved.status, "pending confirmation");
}

async fn email_mock_200_response(test_app: &TestApp) {
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;
}
