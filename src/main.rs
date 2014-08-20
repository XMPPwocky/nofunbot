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

pub use std::collections::HashMap;
pub use usermanager::UserState;

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
  Duration,
  UTC
};

mod banmanager;
mod channelmanager;
mod private; // private data
mod rules;
mod usermanager;

fn main() {
  info!("nofunbot starting up...");

  NoFunBot::launch(Config { 
    nick: "NoFunBot".to_string(),
    server: "irc.quakenet.org".to_string(),
    port: 6667,
    nspass: private::NICKSERV_PASSWORD.to_string(),
  });
}
pub enum RulesCheckResult {
  Infraction(&'static str),
  RulesOK
}

#[deriving(Clone)]
pub struct Config {
  nick: String,
  server: String,
  port: u16,
  nspass: String,
}

pub struct NoFunBot {
  config: Config,
  banmgr: banmanager::BanManager,
  chanmgr: channelmanager::ChannelManager,
  usermgr: usermanager::UserManager
}

impl NoFunBot {
  pub fn launch(config: Config) {
    let mut bot = NoFunBot {
      config: config.clone(),
      banmgr: banmanager::BanManager::new(),
      chanmgr: channelmanager::ChannelManager::new(),
      usermgr: usermanager::UserManager::new()
    };

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
    // clear expired bans, etc.
    self.banmgr.update(conn);

    match line {
      Line{command: IRCCode(1), ..} => {
        info!("Connected, IDing with nickserv");
        conn.privmsg(b"Q@CServe.quakenet.org", format!("AUTH {} {}",
                                                       self.config.nick,
                                                       self.config.nspass
                                                       ).as_bytes());

        self.chanmgr.join_channels(conn);
      },
      Line{command: IRCCode(353), ref args, ..} => {
        // NAMES
        // first 3 args are our nick, "=", channel name
        args.as_slice().get(3).map(|names_bytes| String::from_utf8_lossy(names_bytes.as_slice()).to_string())
          .map(|names|
               for name in names.as_slice().split(' ').map(|s| regex!(r"^[@+]").replace_all(s, "")) { // space delimited
                 self.chanmgr.handle_join(String::from_utf8_lossy(args[2].as_slice()).as_slice(), name.as_slice())
               });
      }
      Line{command: IRCCmd(cmd), args, prefix: prefix } => match cmd.as_slice() {
        "JOIN" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            let nick_bytes = prefix.nick();
            let nick = String::from_utf8_lossy(nick_bytes);
            let nick = nick.as_slice(); // borrow checker malarkey
            let userstate = self.usermgr.get_or_create(nick);
            self.chanmgr.handle_join(String::from_utf8_lossy(args[0].as_slice()).as_slice(), nick);
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
          self.chanmgr.join_ok(chan.as_slice());
        },
        "PART" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            info!("{} left channel", String::from_utf8_lossy(prefix.nick()).to_string());
            self.chanmgr.handle_part(
              String::from_utf8_lossy(args[0].as_slice()).as_slice(),
              String::from_utf8_lossy(prefix.nick()).as_slice()
            );
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

    if ["Crate", "goBot", "face", "YouTube", "weeedbot"].iter().find(|&&n| n == src.as_slice()).is_some() {
      debug!("ignoring bot...");
      return;
    }

    if dst.as_slice().starts_with("#") {
      let valid_command = if msg.as_slice().starts_with(self.config.nick.as_slice()) {
        // we are being addressed!
        // m'lady
        
        // split on ' ', ignoring superfluous whitespace
        let mut args: Vec<&str> = msg.as_slice().trim_chars(' ')
          .split(' ').collect();

        debug!("{}", args);

        // "NoFunBot:"
        if args.len() > 0 {
          *args.get_mut(0) = args.get(0).trim_right_chars(':');
        }

        let mynick = self.config.nick.as_slice();
        match args.as_slice() {
          [mynick, "stopword", word] => {
            if self.chanmgr.nick_in_control_channels(src.as_slice()) {
              //self.stopword(word)
              true
            } else {
              false
            }
          },
          [mynick, "forgive", target_nick] => {
            if self.chanmgr.nick_in_control_channels(src.as_slice()) {
              info!("Forgiving {} by {}'s request...", target_nick, src)
              self.chanmgr.log_to_control_channels(conn, format!("{} forgave {}...", src, target_nick).as_slice());
              self.usermgr.get_or_create(target_nick).infractions = 0;
              true
            } else {
              false
            }
          },
          [mynick, "ban_length", len_str] => {
            if self.chanmgr.nick_in_control_channels(src.as_slice()) {
              match std::from_str::FromStr::from_str(len_str) {
                Some(len) => {
                  self.banmgr.set_ban_length(Duration::minutes(len));
                  self.chanmgr.log_to_control_channels(conn, format!("{} set ban length to {}m", src, len_str).as_slice());
                  true
                },
                None => {
                  warn!("Invalid ban length!");
                  false
                }
              }
            } else {
              false
            }
          },
          _ => {
            warn!("Unknown command from {}: {}", src, msg);
            false
          }
        }
      } else {
        false
      };

      if !valid_command {
        self.moderate(conn, src, dst.as_slice(), msg)
      }
    }
  }
  pub fn moderate(&mut self, conn: &mut Conn, nick: String, channel: &str, msg: String) {
    let userstate = self.usermgr.get_or_create(nick.as_slice());
    
    match rules::check(msg.as_slice(), userstate) {
      Infraction(warn_msg) => {
        // that's a paddlin'
        userstate.infractions += 1;
        info!("{} now has {} infractions...", nick, userstate.infractions);

        if userstate.infractions < 3 {
          // let them off w/ a warning
          conn.privmsg(nick.as_bytes(), format!("{} Please read the channel rules: http://goo.gl/4T6EZR . After {} more infraction{}, you will be banned for {}m!",
                                                warn_msg,
                                                3 - userstate.infractions,
                                                if (3 - userstate.infractions == 1) {""} else {"s"},
                                                self.banmgr.get_ban_length().num_minutes()
                                               ).as_bytes());
          self.chanmgr.log_to_control_channels(conn, format!("Warning {}: {} {} infractions.", nick, warn_msg, userstate.infractions).as_slice()); 
        } else {
          info!("Kicking!");

          userstate.infractions = 0;
          userstate.ban_expiration = Some(chrono::UTC::now() + chrono::duration::Duration::minutes(30));
          self.chanmgr.log_to_control_channels(conn, format!("Banning {}: {}", nick, warn_msg).as_slice());
          self.banmgr.ban(conn, channel, nick.as_slice());
        }
      },
      RulesOK => ()
    }

    userstate.last_message_time = chrono::UTC::now();
    userstate.last_message = msg;
  }
}
