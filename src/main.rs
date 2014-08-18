#![feature(phase)]

#[phase(plugin, link)]
extern crate log;

#[phase(plugin, link)]
extern crate regex_macros;
extern crate regex;

extern crate flate;
extern crate chrono;

extern crate irc = "rust-irclib";

use std::str::IntoMaybeOwned;
use std::collections::HashMap;

use irc::conn::{
  Conn,
  Event,
  IRCCmd,
  IRCCode,
  IRCAction,
  Line,
};

use chrono::{
  DateTime,
  UTC
};

mod rules;

fn main() {
  info!("nofunbot starting up...");

  NoFunBot::launch(Config { 
    nick: "NoFunBot".to_string(),
    server: "irc.quakenet.org".to_string(),
    port: 6667,

    channels: vec!(
      Channel {
        chantype: Moderate,
        name: "#r/globaloffensive".to_string()
      },
      Channel {
        chantype: Control,
        name: "#gobotmods".to_string()
      }
      )
  });
}
pub enum RulesCheckResult {
  Infraction(&'static str),
  RulesOK
}

#[deriving(Clone, PartialEq)]
pub enum ChannelType {
  Moderate, // we mod this channel
    Control // we are controlled here
}
#[deriving(Clone)]
pub struct Channel {
  name: String,
  chantype: ChannelType
}
#[deriving(Clone)]
pub struct Config {
  nick: String,
  server: String,
  port: u16,

  channels: Vec<Channel>
}

pub struct UserState {
  infractions: u32,

  ban_expiration: Option<DateTime<UTC>>,

  last_message_time: DateTime<UTC>,
  last_message: String,

  // consecutive "one word per line" messages
  simple_msg_count: u32
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
pub struct NoFunBot {
  config: Config,
  users: HashMap<String, UserState> 
}

pub fn log_to_control_channels(config: &Config, conn: &mut Conn, msg: &str) {
  for channel in config.channels.iter().filter(|chan| chan.chantype == Control) {
    conn.privmsg(channel.name.as_bytes(), msg.as_bytes());
  }
}
impl NoFunBot {
  pub fn launch(config: Config) {
    let mut bot = NoFunBot { config: config.clone(), users: HashMap::new() };

    let mut ircopts = irc::conn::Options::new(config.server.as_slice(), config.port);
    ircopts.nick = config.nick.as_slice();

    match irc::conn::connect(ircopts, (), |c,e,_| bot.handle(c, e)) {
      Ok(()) => info!("Exiting normally..."),
      Err(err) => error!("Connection error: {}", err)
    }
  }
  pub fn handle(&mut self, conn: &mut Conn, event: Event) {
    match event {
      irc::conn::Connected => info!("Connected"),
      irc::conn::Disconnected => info!("Disconnected"),
      irc::conn::LineReceived(line) => self.handle_line(conn, line)
    }
  }
  pub fn handle_line(&mut self, conn: &mut Conn, line: Line) {
    match line {
      Line{command: IRCCode(1), ..} => {
        info!("Logged in");
        for channel in self.config.channels.iter() {
          conn.join(channel.name.as_bytes(), [])
        }
      },
      Line{command: IRCCode(353), ref args, ..} => {
        // NAMES
        // first 3 args are our nick, "=", channel name
        args.as_slice().get(3).map(|names_bytes| String::from_utf8_lossy(names_bytes.as_slice()).to_string())
          .map(|names|
               for name in names.as_slice().split(' ').map(|s| regex!(r"^[@+]").replace_all(s, "")) { // space delimited
                 self.add_user(name)
               });
      }
      Line{command: IRCCmd(cmd), args, prefix: prefix } => match cmd.as_slice() {
        "JOIN" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            let nick = String::from_utf8_lossy(prefix.nick()).to_string();
            let userstate = self.users.find_or_insert_with(nick.clone(), |_| UserState::new());
            match userstate.ban_expiration { 
              Some(exptime) if exptime > chrono::UTC::now() => {
                let kickmsg = format!("You are still banned for {} seconds!", (exptime - chrono::UTC::now()).num_seconds());
                conn.send_command(IRCCmd("KICK".into_maybe_owned()),
                                  [args.move_iter().next().unwrap(), nick.clone().into_bytes(), kickmsg.into_bytes()], true);
              }, 
                _ => ()
            }
            return;
          }
          if args.is_empty() {
            let line = Line{command: IRCCmd("JOIN".into_maybe_owned()),
            args: args, prefix: Some(prefix)};
            error!("ERROR: Invalid JOIN message received: {}", line);
            return;
          }
          let chan = args.move_iter().next().unwrap();
          let chan = String::from_utf8_lossy(chan.as_slice());
          info!("JOINED: {}", chan);
        },
        "PART" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            info!("{} left channel", String::from_utf8_lossy(prefix.nick()).to_string());
            //self.remove_user(String::from_utf8_lossy(prefix.nick()).to_string())
          }
        },
        "PRIVMSG" | "NOTICE" => {
          let (src, dst, msg) = match prefix {
            Some(_) if args.len() == 2 => {
              let mut args = args;
              let (dst, msg) = (args.swap_remove(0).unwrap(),
              args.move_iter().next().unwrap());
              (prefix.as_ref().unwrap().nick(), dst, msg)
            }
            _ => {
              error!("ERROR: Unexpected {} line: ", cmd);
              let line = Line{command: IRCCmd(cmd), args: args, prefix: prefix};
              error!("{}", line);
              return;
            }
          };
          let dsts = String::from_utf8_lossy(dst.as_slice()).into_string();
          let srcs = String::from_utf8_lossy(src.as_slice()).into_string();
          let msgs = String::from_utf8_lossy(msg.as_slice()).into_string();
          self.handle_privmsg(conn, msgs, srcs, dsts)
        }
        _ => ()
      },
      Line{command: IRCAction(dst), args, prefix } => {
        let (src, msg) = match prefix {
          Some(_) if args.len() == 1 => {
            let msg = args.move_iter().next().unwrap();
            (prefix.as_ref().unwrap().nick(), msg)
          }
          _ => {
            let line = Line{command: IRCAction(dst), args: args, prefix: prefix};
            error!("ERROR: Unexpected ACTION line: {}", line);
            return;
          }
        };
        let dst = String::from_utf8_lossy(dst.as_slice());
        let src = String::from_utf8_lossy(src.as_slice());
        let msg = String::from_utf8_lossy(msg.as_slice());
      }
      _ => ()
    }
  }
  pub fn handle_privmsg(&mut self, conn: &mut Conn, msg: String, src: String, dst: String) {
    info!("{} -> {}: {}", src, dst, msg);

    if dst.as_slice().starts_with("#") {
      let maybe_chan = self.config.channels.iter().find(|chan| chan.name == dst).map(|x| x.clone());
      match maybe_chan {
        Some(channel) => {
          self.moderate(conn, src, &channel, msg)
        },
        None => {
          debug!("Silently ignoring...");
        }
      }
    }
  }
  pub fn moderate(&mut self, conn: &mut Conn, nick: String, channel: &Channel, msg: String) {
    if ["Crate", "goBot", "face", "YouTube", "weeedbot"].iter().find(|&&n| n == nick.as_slice()).is_some() {
      debug!("ignoring bot...");
      return;
    }

    let &NoFunBot{users: ref mut users, config: ref mut cfg} = self;
    let userstate = users.find_or_insert_with(nick.clone(), |_| UserState::new());
    
    match rules::check(msg.as_slice(), userstate) {
      Infraction(warn_msg) => {
        // that's a paddlin'
        userstate.infractions += 1;
        info!("{} now has {} infractions...", nick, userstate.infractions);

        if userstate.infractions < 3 {
          // let them off w/ a warning
          conn.privmsg(nick.as_bytes(), format!("{} Please read the channel rules: http://goo.gl/4T6EZR . {} warning{} left until you are banned.",
                                                warn_msg,
                                                2 - userstate.infractions,
                                                if (2 - userstate.infractions == 1) {""} else {"s"}
                                               ).as_bytes());
          log_to_control_channels(cfg, conn, format!("Warning {}: {} {} infractions.", nick, warn_msg, userstate.infractions).as_slice()); 
        } else {
          info!("Kicking!");

          userstate.infractions = 0;
          userstate.ban_expiration = Some(chrono::UTC::now() + chrono::duration::Duration::minutes(30));
          log_to_control_channels(cfg, conn, format!("Banning {}: {}", nick, warn_msg).as_slice());
          conn.send_command(IRCCmd("KICK".into_maybe_owned()),
                            [channel.name.as_bytes(), nick.as_bytes(), b"Banned for 30min"], true);
        }
      },
      RulesOK => ()
    }

    userstate.last_message_time = chrono::UTC::now();
    userstate.last_message = msg;
  }
  pub fn add_user(&mut self, nick_str: String) {
    //let nick_str = String::from_utf8_lossy(user.nick().as_slice()).to_string();
    info!("Adding user w/ nick {}", nick_str);
    self.users.insert(nick_str, UserState::new());
    debug!("I know about {} users", self.users.len());
  }
  /*
     pub fn remove_user(&mut self, nick_str: String) {
//let nick_str = String::from_utf8_lossy(user.nick().as_slice()).to_string();
info!("Removing user w/ nick {}", nick_str);
self.users.remove(&nick_str);
debug!("{} users left...", self.users.len());
}
   */
}
