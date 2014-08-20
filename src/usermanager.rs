use chrono;
use chrono::{
  DateTime,
  UTC
};
use std::collections::HashMap;

pub struct UserState {
  pub infractions: u32,

  pub ban_expiration: Option<DateTime<UTC>>,

  pub last_message_time: DateTime<UTC>,
  pub last_message: String,

  // consecutive "one word per line" messages
  pub simple_msg_count: u32
}
impl UserState {
  pub fn new() -> UserState {
    UserState {
      infractions: 0,

      last_message: "".to_string(),
      last_message_time: chrono::UTC::now(),

      ban_expiration: None,

      simple_msg_count: 0
    }
  }
}

pub struct UserManager {
  users: HashMap<String, UserState>
}
impl UserManager {
  pub fn new() -> UserManager {
    UserManager { users: HashMap::new() }
  }
  /// Either gets existing UserState for a nick,
  /// or creates a new one for you.
  pub fn get_or_create<'a>(&'a mut self, nick: &str) -> &'a mut UserState {
    // no find_mut_equiv? ;_;
    let nick = nick.to_string();
    self.users.find_or_insert_with(nick, |_| UserState::new())
  }
}
