#![feature(phase)]

#[phase(plugin, link)]
extern crate log;

#[phase(plugin, link)]
extern crate regex_macros;
extern crate regex;

extern crate time;

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

mod rules;

fn main() {
  info!("nofunbot starting up...");

  NoFunBot::launch(Config { 
    nick: "NoFunBot".to_string(),
    server: "irc.quakenet.org".to_string(),
    port: 6667,

    patience: 5,
    annoyance_cooldown: 30,

    channels: vec![Channel {
      chantype: Moderate,
      name: "#nofunbot".to_string() 
    }]
  });
}
#[deriving(Clone)]
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
  
  patience: u32, // max patience
  annoyance_cooldown: u32,

  channels: Vec<Channel>
}

pub struct UserState {
  annoyance: u32,
  last_annoyance: i64 // unix time
}

pub struct NoFunBot {
  config: Config,
  users: HashMap<String, UserState> 
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
        args.as_slice().get(3).map(|names_bytes| String::from_utf8_lossy(names_bytes.as_slice()).to_owned())
          .map(|names|
               for name in names.as_slice().split(' ') { // space delimited
                 self.add_user(name.to_owned());
               });
      }
      Line{command: IRCCmd(cmd), args, prefix: prefix } => match cmd.as_slice() {
        "JOIN" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            self.add_user(String::from_utf8_lossy(prefix.nick()).to_owned());
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
            self.remove_user(String::from_utf8_lossy(prefix.nick()).to_owned());
            return;
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
    let userstate = self.users.get_mut(&nick);
    
    let cooldowns_elapsed = (time::get_time().sec - userstate.last_annoyance) as u32 / self.config.annoyance_cooldown;
    if cooldowns_elapsed > 0 {
      userstate.annoyance /= cooldowns_elapsed;
    }
    
    let score = rules::score(&msg);
    if score == 0 { return }
    else { // you dun goofed
      // TODO: check for op
      conn.privmsg(channel.name.as_bytes(), format!("{}: Please read the channel rules: http://goo.gl/4T6EZR . If you do not follow these rules, you may be kicked!", nick).as_bytes());
      userstate.annoyance += score;
      userstate.last_annoyance = time::get_time().sec;
    }
    
    info!("Annoyance level of {}: {}", nick, userstate.annoyance);

    if userstate.annoyance > self.config.patience {
      info!("Kicking!");
    }
    //conn.send_command(IRCCmd("KICK".into_maybe_owned()),
      //                  [channel.name.as_bytes(), nick.as_bytes(), b"Source was the best CS."], true);
  }
  pub fn add_user(&mut self, nick_str: String) {
    //let nick_str = String::from_utf8_lossy(user.nick().as_slice()).to_owned();
    info!("Adding user w/ nick {}", nick_str);
    self.users.insert(nick_str, UserState { annoyance: 0, last_annoyance: 0 });
    debug!("I know about {} users", self.users.len());
  }
  pub fn remove_user(&mut self, nick_str: String) {
    //let nick_str = String::from_utf8_lossy(user.nick().as_slice()).to_owned();
    info!("Removing user w/ nick {}", nick_str);
    self.users.remove(&nick_str);
    debug!("{} users left...", self.users.len());
  }
}
