use crate::models::payload::Payload;
use crate::models::user::User;
use chrono::Local;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub id: Uuid,
    pub user: User,
    pub channel: u32,
    pub content: String,
    pub time: String,
}

impl Message {
    pub fn new(user: &mut User, content: &str, channel: u32) -> Self {
        user.update_last_message_time();
        let time_str = Local::now().format("%H:%M  ").to_string();
        let message_id = Uuid::new_v4();
        Self { id: message_id, user: user.clone(), channel, content: content.to_string(), time: time_str }
    }

    pub fn from_payload(payload: Payload) -> Self {
        Self::new(&mut User::new(payload.get_username()), &*String::from_utf8_lossy(payload.get_data()).trim(), payload.get_channel())
    }

    pub fn get_username(&self) -> &str {
        self.user.get_username()
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }

    pub fn get_time(&self) -> &str {
        &self.time
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }
}
