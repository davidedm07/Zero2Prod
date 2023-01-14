use crate::domain::SubscriberEmail;
use reqwest::Client;

pub struct EmailClient {
    sender: SubscriberEmail,
    client: Client,
    base_url: String,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail) -> Self {
        Self {
            sender,
            client: Client::new(),
            base_url,
        }
    }

    pub fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        text_content: &str,
        html_content: &str,
    ) -> Result<(), String> {
        todo!()
    }
}
