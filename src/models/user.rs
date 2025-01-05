use chrono::{DateTime, Local};

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    username: String,
    time_created: DateTime<Local>,
    time_last_message: DateTime<Local>,
}

impl User {
    pub fn new(username: &str) -> Self {
        let now = Local::now();
        Self { username: username.to_string(), time_created: now, time_last_message: now }
    }

    pub fn update_last_message_time(&mut self) {
        self.time_last_message = Local::now();
    }

    pub fn get_username(&self) -> &str {
        &self.username
    }

    pub fn get_time_created(&self) -> DateTime<Local> {
        self.time_created
    }

    pub fn get_time_last_message(&self) -> DateTime<Local> {
        self.time_last_message
    }
}
