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
    self.channels.insert(name.to_string(), IRCChannel {
      name: name.to_string(),
      chantype: chantype,
      nicks: HashSet::new(),
      joined: false
    });
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
  /// Is a given nick in any control channels? (etc. an operator)
  pub fn nick_in_control_channels(&self, nick: &str) -> bool {
    for (_, chan) in self.channels.iter().filter(|&(_, s)| s.chantype == Control) {
      if chan.contains_nick(nick) {
        return true;
      }
    }
    false
  }
}

impl IRCChannel {
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
}
