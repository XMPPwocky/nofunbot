use std::collections::{HashMap, HashSet};
use irc::conn::Conn;

#[deriving(Clone, PartialEq)]
pub enum ChannelType {
  Moderate, // we mod this channel
    Control // we are controlled here
}
#[deriving(Clone)]
pub struct ChannelState {
  chantype: ChannelType,

  nicks: HashSet<String>,
  joined: bool
}

/// keeps track of all the channels we're in,
/// as well as nicks in them.
pub struct ChannelManager {
  channels: HashMap<String, ChannelState>
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
    self.channels.insert(name.to_string(), ChannelState {
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
      conn.join(name.as_bytes(), [])
    }
  }

  /// We have successfully joined a channel! Hooray!
  pub fn join_ok(&mut self, name: &str) {
    match self.channels.find_mut(&name.to_string()) {
      Some(chan) => { chan.joined = true },
      None => { error!("Got join_ok called, but {} not in self.channels?!?", name) }
    }
  }

  /// Handles somebody else (NOT us) joining a channel.
  pub fn handle_join(&mut self, chan: &str, nick: &str) {
    let nick = nick.to_string();
    match self.channels.find_mut(&chan.to_string()) {
      Some(chan) => {chan.nicks.insert(nick);},
      None => error!("Trying to handle a join for {} into {}, but channel not in self.channels!", nick, chan)
    }
  }

  /// Handles somebody else (NOT us) leaving a channel.
  pub fn handle_part(&mut self, chan: &str, nick: &str) {
    let nick = nick.to_string();
    match self.channels.find_mut(&chan.to_string()) {
      Some(chanstate) => match chanstate.nicks.remove(&nick) {
        true => (),
        false => error!("{} parted from {}, but was never in nicks!", nick, chan)
      },
      None => error!("Trying to handle a part for {} from {}, but channel not in self.channels!", nick, chan)
    }
  }
  /// Prints to all control channels.
  pub fn log_to_control_channels(&self, conn: &mut Conn, msg: &str) {
    for (name, _) in self.channels.iter().filter(|&(_, s)| s.chantype == Control) {
      conn.privmsg(name.as_bytes(), msg.as_bytes());
    }
  }
  /// Is a given nick in any control channels? (e.g. an operator)
  pub fn nick_in_control_channels(&self, nick: &str) -> bool {
    for (_, state) in self.channels.iter().filter(|&(_, s)| s.chantype == Control) {
      if state.nicks.contains_equiv(&nick) {
        return true;
      }
    }
    false
  }
}
