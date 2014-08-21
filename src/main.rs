#![feature(phase)]

#[phase(plugin, link)]
extern crate log;

#[phase(plugin, link)]
extern crate regex_macros;
extern crate regex;

extern crate flate;
extern crate chrono;
extern crate serialize;

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

use chrono::Duration;

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

#[deriving(Clone, Decodable)]
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
      },
      Line{command: IRCCode(353), ref args, ..} => {
        // NAMES
        // first 3 args are our nick, "=", channel name
        args.as_slice().get(3).map(|names_bytes| String::from_utf8_lossy(names_bytes.as_slice()).to_string())
          .map(|names|
               for name in names.as_slice().split(' ').map(|s| regex!(r"^[@+]").replace_all(s, "")) { // space delimited
                 self.chanmgr.find_mut(String::from_utf8_lossy(args[2].as_slice()).as_slice())
                   .map(|chan| chan.handle_join(name.as_slice()));
               });
      }
      Line{command: IRCCmd(cmd), args, prefix: prefix } => match cmd.as_slice() {
        "JOIN" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            let nick_bytes = prefix.nick();
            let nick = String::from_utf8_lossy(nick_bytes);
            let nick = nick.as_slice(); // borrow checker malarkey
            //let userstate = self.usermgr.get_or_create(nick);
            self.chanmgr.find_mut(String::from_utf8_lossy(args[0].as_slice()).as_slice())
              .map(|chan| chan.handle_join(nick));
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
          self.chanmgr.find_mut(chan.as_slice()).map(|chan| chan.join_ok());
        },
        "PART" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            info!("{} left channel", String::from_utf8_lossy(prefix.nick()).to_string());
            self.chanmgr.find_mut(String::from_utf8_lossy(args[0].as_slice()).as_slice())
              .map(|chan| chan.handle_part(String::from_utf8_lossy(prefix.nick()).as_slice()));
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
          if msgs.as_slice().starts_with("You are now logged in as") {
            info!("NickServ OK, joining channels");
            self.chanmgr.join_channels(conn);
          } else {
            self.handle_privmsg(conn, msgs, srcs, dsts)
          }
        }
        _ => ()
      },
      Line{command: IRCAction(dst), args, prefix } => {
        match prefix {
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
        //let dst = String::from_utf8_lossy(dst.as_slice());
        //let src = String::from_utf8_lossy(src.as_slice());
        //let msg = String::from_utf8_lossy(msg.as_slice());
        warn!("Ignoring action (not implemented)");
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
          *args.get_mut(0) = args[0].trim_right_chars(':');
        }

        match args.as_slice() {
          [_mynick, "stopword", ..words] => {
            if self.chanmgr.nick_in_control_channels(src.as_slice()) {
              let word = words.iter().fold(String::new(), |state, elem| state.append(*elem));

              conn.privmsg(dst.as_bytes(), format!("Okay, {}, next person to say {} gets kickbanned!",
                                                   src,
                                                   word).as_slice().as_bytes());

              self.chanmgr.find_mut(dst.as_slice()).expect("Channel not found!").set_stopword(Some(word));

              true
            } else {
              false
            }
          },
          [_mynick, "forgive", target_nick] => {
            if self.chanmgr.nick_in_control_channels(src.as_slice()) {
              info!("Forgiving {} by {}'s request...", target_nick, src)
              self.chanmgr.log_to_control_channels(conn, format!("{} forgave {}...", src, target_nick).as_slice());
              self.usermgr.get_or_create(target_nick).infractions = 0;
              true
            } else {
              false
            }
          },
          [_mynick, "ban_length", len_str] => {
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
    // early stopword check
    let stopword_detected = match self.chanmgr.find(channel).and_then(|ch| ch.get_stopword()) {
      Some(stopword) if msg.as_slice().contains(stopword) => true,
      _ => false
    };
    if stopword_detected {
      self.chanmgr.log_to_control_channels(conn, format!("Banned {} for stopword violation", nick).as_slice());
      self.banmgr.ban(conn, channel, nick.as_slice());
      self.chanmgr.find_mut(channel).map(|ch| ch.set_stopword(None));
    };

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
                                                if 3 - userstate.infractions == 1 {""} else {"s"},
                                                self.banmgr.get_ban_length().num_minutes()
                                               ).as_bytes());
          self.chanmgr.log_to_control_channels(conn, format!("Warning {}: {} {} infractions.", nick, warn_msg, userstate.infractions).as_slice()); 
        } else {
          info!("Kicking!");

          userstate.infractions = 0;
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
