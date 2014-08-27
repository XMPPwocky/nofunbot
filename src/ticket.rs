use irc;
use banmanager::Ban;
use chrono::Duration;

pub struct Ticket {
  info_msg: String,
  channel: String,
  user: irc::User
}
impl Ticket {
  pub fn new(self, channel: String, user: irc::User, info_msg: String) -> Ticket {
    Ticket {
      channel: channel,
      user: user,
      info_msg: info_msg
    }
  }

  /// Note: the ban is NOT applied for you!
  pub fn to_ban(self, length: Duration) -> Ban {
    Ban::new(self.channel.as_slice(), &self.user, length)
  }
}
