use crate::helpers::spawn_app;

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=Jon%20Doe&email=jondoe%40email.com";
    let response = client
        .post(&format!("{}/subscriptions", &test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute requets");

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_returns_400_when_data_is_missing_or_empty() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("", "missing email and name"),
        ("name=Jon", "missing email"),
        ("email=jon%40email.com", "missing name"),
        ("name=&email=jon%40email.com", "empty name"),
        ("name=Jon&email=", "empty email"),
        ("name=Jon&email=not-an-email", "invalid email"),
    ];

    for (body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &test_app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute requets");

        assert_eq!(
            400,
            response.status().as_u16(),
            "API did not fail with 400 bad request when payload was {}.",
            error_message
        );
    }
}
