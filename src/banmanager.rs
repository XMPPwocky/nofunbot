use chrono::{Duration, DateTime, UTC};
use irc::conn::{Conn,IRCCmd};

struct Ban {
  mask: String,
  channel: String,
  expires: DateTime<UTC>
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
    let expired_bans: Vec<uint> = { self.bans.iter().filter(|ban| ban.expires < now).enumerate().map(|(id, _)| id).collect() };
    for id in expired_bans.iter() {
      self.unban(conn, *id)
    }
  }

  /// Bans a nick. TODO: this can't extend existing bans
  pub fn ban(&mut self, conn: &mut Conn, channel: &str, nick: &str) {
    //conn.send_command(IRCCmd("KICK".into_maybe_owned()),
    //  [channel.as_bytes(), nick.as_bytes(), b"Temp-banned"], true);

    let banmask = format!("{}!*@*", nick);
    info!("Setting +b on {}", banmask);

    conn.send_command(IRCCmd("MODE".into_maybe_owned()), [channel.as_bytes(), b"+b", banmask.as_bytes()], false);
    
    self.bans.push(Ban {
      mask: banmask,
      channel: channel.to_string(),
      expires: UTC::now() + self.ban_length
    });
  }
  pub fn unban(&mut self, conn: &mut Conn, id: uint) {
    let Ban { mask: mask, channel: channel, ..} = self.bans.remove(id).unwrap();
    conn.send_command(IRCCmd("MODE".into_maybe_owned()), [channel.as_bytes(), b"-b", mask.as_bytes()], false);
  }
  pub fn set_ban_length(&mut self, length: Duration) {
    self.ban_length = length;
  }
  pub fn get_ban_length(&self) -> Duration {
    self.ban_length
  }
}
