use once_cell::sync::Lazy;
use reqwest::{Client, Response, Url};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::startup::{get_connection_pool, Application};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_connection_pool: PgPool,
    pub email_server: MockServer,
}

pub struct ConfirmationLinks {
    pub html: Url,
    pub plain_text: Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let json_body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = Url::parse(&raw_link).unwrap();
            assert_eq!("127.0.0.1", confirmation_link.host_str().unwrap());
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html_link = get_link(&json_body["HtmlBody"].as_str().unwrap());
        let text_link = get_link(&json_body["TextBody"].as_str().unwrap());

        ConfirmationLinks {
            html: html_link,
            plain_text: text_link,
        }
    }

    pub async fn call_confirmation_link(&self) -> Response {
        let email_request = &self.email_server.received_requests().await.unwrap()[0];
        let confirmation_links = self.get_confirmation_links(email_request);

        let html_link = confirmation_links.html;
        assert_eq!("127.0.0.1", html_link.host_str().unwrap());

        reqwest::get(html_link)
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
    }

    pub async fn email_mock_200_response(&self) {
        Mock::given(path("/email"))
            .and(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&self.email_server)
            .await;
    }
}

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url =
            Url::parse(email_server.uri().as_str()).expect("Failed to parse URL");
        c
    };

    configure_database(&configuration.database).await;

    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");

    let application_port = application.port();
    let address = format!("http://localhost:{}", application_port);
    let _ = tokio::spawn(application.run_until_stopped());

    TestApp {
        address,
        port: application_port,
        db_connection_pool: get_connection_pool(&configuration.database),
        email_server,
    }
}

pub async fn configure_database(configuration: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&configuration.without_db())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE database "{}""#, configuration.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPool::connect_with(configuration.with_db())
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}
