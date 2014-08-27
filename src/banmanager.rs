use chrono::{Duration, DateTime, UTC};
use irc::conn::{Conn,IRCCmd};
use irc;

pub struct Ban {
  mask: String,
  channel: String,
  expires: DateTime<UTC>
}
impl Ban {
  pub fn new(channel: &str, user: &irc::User, length: Duration) -> Ban {
    let banmask = format!("*!*@{}", String::from_utf8_lossy(user.host().expect("No hostname?")));
    Ban { 
      mask: banmask,
      channel: channel.to_string(),
      expires: UTC::now() + length
    }
  }
  pub fn is_active(&self) -> bool {
    let curtime = UTC::now();
    
    self.expires >= curtime
  }
  /// Updates the modes on the channel to reflect this ban.
  pub fn update_usermode(&self, conn: &mut Conn) {
    if self.is_active() {
      conn.send_command(IRCCmd("MODE".into_maybe_owned()), [self.channel.as_bytes(), b"+b", self.mask.as_bytes()], false);
    } else {
      conn.send_command(IRCCmd("MODE".into_maybe_owned()), [self.channel.as_bytes(), b"-b", self.mask.as_bytes()], false);
    }
  }
}

pub struct BanManager {
  bans: Vec<Ban>,
  ban_length: Duration
}
impl BanManager {
  pub fn new() -> BanManager {
    BanManager { bans: Vec::new(), ban_length: Duration::minutes(5) }
  }
  /// Unbans expired bans
  pub fn update(&mut self, conn: &mut Conn) {
    let now = UTC::now();
    let expired_bans: Vec<uint> = { self.bans.iter().filter(|ban| !ban.is_active()).enumerate().map(|(id, _)| id).collect() };
    for id in expired_bans.iter() {
      self.unban(conn, *id)
    }
  }

  /// Bans a nick. TODO: this can't extend existing bans
  pub fn ban(&mut self, conn: &mut Conn, channel: &str, user: &irc::User) {
    //conn.send_command(IRCCmd("KICK".into_maybe_owned()),
    //  [channel.as_bytes(), nick.as_bytes(), b"Temp-banned"], true);

    let ban = Ban::new(channel, user, self.ban_length);
    ban.update_usermode(conn);
    
    self.bans.push(ban);
  }
  pub fn unban(&mut self, conn: &mut Conn, id: uint) {
    let expired_ban = self.bans.remove(id).unwrap();
    expired_ban.update_usermode(conn);
  }
  pub fn set_ban_length(&mut self, length: Duration) {
    self.ban_length = length;
  }
  pub fn get_ban_length(&self) -> Duration {
    self.ban_length
  }
}
