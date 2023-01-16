use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let configuration = get_configuration().expect("Failed to read configuration");

    let connection_pool = PgPool::connect_lazy_with(configuration.database.with_db());

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address");
    let url = EmailClient::parse_url(configuration.email_client.base_url);
    let email_client = EmailClient::new(
        url,
        sender_email,
        configuration.email_client.authorization_token,
        std::time::Duration::from_millis(configuration.email_client.timeout_milliseconds),
    );

    let listener = TcpListener::bind(&format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    ))
    .expect("Failed to bind random port");
    println!(
        "Server will be running on port: {}",
        listener.local_addr().unwrap().port()
    );
    run(listener, connection_pool, email_client)?.await
}
