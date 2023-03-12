use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, TestApp};

#[tokio::test]
async fn newsletter_are_not_delivered_to_unconfirmed_subscribers() {
    let test_app = spawn_app().await;
    create_unconfirmed_subscriber(&test_app).await;

    test_app.email_mock_200_response_with_times(0).await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text":"Newsletter body as plain text",
            "html":"<p> Newsletter body as HTML </p>"
        }
    });

    let response = test_app.post_newsletters(newsletter_request_body).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let test_app = spawn_app().await;
    create_unconfirmed_subscriber(&test_app).await;
    test_app.call_confirmation_link().await;
    test_app.email_mock_200_response_with_times(1).await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text":"Newsletter body as plain text",
            "html":"<p> Newsletter body as HTML </p>"
        }
    });

    let response = test_app.post_newsletters(newsletter_request_body).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let test_app = spawn_app().await;

    let test_cases = vec![
        (
            serde_json::json!(
                {
                    "content": {
                        "text":"Newsletter body as plain text",
                        "html":"<p> Newsletter body as HTML </p>"
                    }
                }
            ),
            "missing title",
        ),
        (
            serde_json::json!({"title": "Newsletter title"}),
            "missing content",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = test_app.post_newsletters(invalid_body).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}",
            error_message
        );
    }
}

#[tokio::test]
async fn requests_without_authorization_header_are_rejected() {
    let test_app = spawn_app().await;
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text":"Newsletter body as plain text",
            "html":"<p> Newsletter body as HTML </p>"
        }
    });

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", test_app.address))
        .json(&newsletter_request_body)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(401, response.status().as_u16());
}

#[tokio::test]
async fn non_existing_user_is_rejected() {
    let test_app = spawn_app().await;
    let username = Uuid::new_v4().to_string();
    let password = Uuid::new_v4().to_string();

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text":"Newsletter body as plain text",
            "html":"<p> Newsletter body as HTML </p>"
        }
    });

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", test_app.address))
        .basic_auth(username, Some(password))
        .json(&newsletter_request_body)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

#[tokio::test]
async fn invalid_password_is_rejected() {
    let test_app = spawn_app().await;
    let username = &test_app.test_user.username;
    let password = Uuid::new_v4().to_string();

    assert_ne!(password, test_app.test_user.password);

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text":"Newsletter body as plain text",
            "html":"<p> Newsletter body as HTML </p>"
        }
    });

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", test_app.address))
        .basic_auth(username, Some(password))
        .json(&newsletter_request_body)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

async fn create_unconfirmed_subscriber(test_app: &TestApp) {
    let body = "name=Jon%20Doe&email=jondoe%40email.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&test_app.email_server)
        .await;

    test_app
        .post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();
}
