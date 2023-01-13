use super::SubscriberEmail;
use super::SubscriberName;

#[derive(Debug)]
pub struct Subscriber {
    pub name: SubscriberName,
    pub email: SubscriberEmail,
}
