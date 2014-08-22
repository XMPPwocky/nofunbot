use std::collections::{HashMap, HashSet};
use irc::conn::Conn;

#[deriving(Clone, PartialEq)]
pub enum ChannelType {
  Moderate, // we mod this channel
    Control // we are controlled here
}

#[deriving(Clone)]
pub struct IRCChannel {
  name: String,
  chantype: ChannelType,

  nicks: HashSet<String>,
  joined: bool,
  
  stopword: Option<String>
}

/// keeps track of all the channels we're in,
/// as well as nicks in them.
pub struct ChannelManager {
  channels: HashMap<String, IRCChannel>
}

impl ChannelManager {
  pub fn new_blank() -> ChannelManager {
    ChannelManager { channels: HashMap::new() }
  }
  pub fn new() -> ChannelManager {
    let mut mgr = ChannelManager::new_blank();
    mgr.add_channel("#r/globaloffensive", Moderate);
    mgr.add_channel("#gobotmods", Control);
    mgr
  }

  /// Adds a channel to the list.
  pub fn add_channel(&mut self, name: &str, chantype: ChannelType) {
    self.channels.insert(name.to_string(), IRCChannel::new(name, chantype));
  }

  /// Joins any channels we are not already in.
  /// Note this does not mark the channels as joined,
  /// as we need confirmation from the server.
  pub fn join_channels(&self, conn: &mut Conn) {
    for (name, _) in self.channels.iter().filter(|&(_, c)| !c.joined) {
      conn.join(name.as_slice().as_bytes(), [])
    }
  }
  pub fn find<'a>(&'a self, name: &str) -> Option<&'a IRCChannel> {
    self.channels.find_equiv(&name)
  }
  pub fn find_mut<'a>(&'a mut self, name: &str) -> Option<&'a mut IRCChannel> {
    self.channels.mut_iter().find(|&(ref k, _)| k.as_slice() == name).map(|(_, v)| v)
  }
  /// Prints to all control channels.
  pub fn log_to_control_channels(&self, conn: &mut Conn, msg: &str) {
    for (name, _) in self.channels.iter().filter(|&(_, s)| s.chantype == Control) {
      conn.privmsg(name.as_slice().as_bytes(), msg.as_bytes());
    }
  }
  /// Is a given nick in any control channels? (etc. a mod)
  pub fn nick_is_mod(&self, nick: &str) -> bool {
    for (_, chan) in self.channels.iter().filter(|&(_, s)| s.chantype == Control) {
      if chan.contains_nick(nick) {
        return true;
      }
    }
    false
  }
}

impl IRCChannel {
  fn new(name: &str, chantype: ChannelType) -> IRCChannel {
    IRCChannel {
      name: name.to_string(),
      chantype: chantype,
      nicks: HashSet::new(),
      joined: false,
      stopword: None
    }
  }

  /// We have successfully joined a channel! Hooray!
  pub fn join_ok(&mut self) {
    self.joined = true
  }

  /// Handles somebody else (NOT us) joining a channel.
  pub fn handle_join(&mut self, nick: &str) {
    self.nicks.insert(nick.to_string());
  }

  /// Handles somebody else (NOT us) leaving a channel.
  pub fn handle_part(&mut self, nick: &str) {
    match self.nicks.remove(&nick.to_string()) {
      true => (),
      false => error!("{} parted from {}, but was never in nicks!", self.name, nick)
    }
  }

  pub fn contains_nick(&self, nick: &str) -> bool {
    self.nicks.contains_equiv(&nick)
  }

  pub fn get_stopword<'a>(&'a self) -> Option<&'a str> {
    match self.stopword {
      Some(ref sw) => Some(sw.as_slice()),
      None => None
    }
  }
  pub fn set_stopword<'a>(&mut self, stopword: Option<String>) {
    self.stopword = stopword;
  }
}

#[cfg(test)]
mod test {
  use super::{IRCChannel, Moderate};

  #[test]
  fn nick_tracking() {
    let test_nick = "fredbloggs";

    let mut chan = IRCChannel::new("#test", Moderate);
    assert!(!chan.contains_nick(test_nick));

    chan.handle_join(test_nick);
    assert!(chan.contains_nick(test_nick));
    
    chan.handle_part(test_nick);
    assert!(!chan.contains_nick(test_nick));
  }

  /// Multiple joins should not stack
  #[test]
  fn duplicate_nicks() {
    let test_nick = "fredbloggs";

    let mut chan = IRCChannel::new("#test", Moderate);
    
    for _ in range(0u, 10) {
      chan.handle_join(test_nick);
    }
    assert!(chan.contains_nick(test_nick));
    
    chan.handle_part(test_nick);
    assert!(!chan.contains_nick(test_nick));
  }
}
