use crate::domain::SubscriberEmail;
use reqwest::{Client, Url};
use secrecy::{ExposeSecret, Secret};

pub struct EmailClient {
    sender: SubscriberEmail,
    client: Client,
    base_url: Url,
    authorization_token: Secret<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEMailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

impl EmailClient {
    pub fn new(
        base_url: Url,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            sender,
            client: Client::builder().timeout(timeout).build().unwrap(),
            base_url,
            authorization_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        text_content: &str,
        html_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = self.base_url.join("/email").unwrap();
        let request_body = SendEMailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject: subject,
            html_body: html_content,
            text_body: text_content,
        };
        let _builder = self
            .client
            .post(url)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub fn parse_url(base_url: String) -> Url {
        match Url::parse(&base_url) {
            Ok(url) => url,
            Err(_) => panic!("An invalid Url has been provided to the email client"),
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{domain::SubscriberEmail, email_client::EmailClient};
    use claim::{assert_err, assert_ok};
    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake, Faker,
    };
    use secrecy::Secret;
    use wiremock::{
        matchers::{any, header, header_exists, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    struct SendEmailBodyMatcher;

    // implementing Match trait to check if the body received by the wiremock server
    // contains the expected fields
    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &wiremock::Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("TextBody").is_some()
                    && body.get("HtmlBody").is_some()
            } else {
                false
            }
        }
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn email_client(base_url: String) -> EmailClient {
        let url = EmailClient::parse_url(base_url);
        EmailClient::new(
            url,
            email(),
            Secret::new(Faker.fake()),
            std::time::Duration::from_millis(200),
        )
    }

    #[tokio::test]
    async fn send_email_sends_expected_request() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let content = content();
        let outcome = email_client
            .send_email(&email(), &subject(), &content, &content)
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let content = content();
        let outcome = email_client
            .send_email(&email(), &subject(), &content, &content)
            .await;

        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());
        let response = ResponseTemplate::new(500).set_delay(std::time::Duration::from_secs(120));

        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        let content = content();
        let outcome = email_client
            .send_email(&email(), &subject(), &content, &content)
            .await;

        assert_err!(outcome);
    }
}
