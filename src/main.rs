#![feature(phase)]

#[phase(plugin, link)]
extern crate log;

extern crate irc = "rust-irclib";

use irc::conn::{
  Conn,
  Event,
  IRCCmd,
  IRCCode,
  IRCAction,
  Line,
};

fn main() {
  info!("nofunbot starting up...");

  NoFunBot::launch(Config { 
    nick: "NoFunBot".to_string(),
    server: "irc.quakenet.org".to_string(),
    port: 6667,

    channels: vec![Channel {
      chantype: Moderate,
      name: "#r/globaloffensive".to_string() 
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

  channels: Vec<Channel>
}

pub struct NoFunBot {
  config: Config
}

impl NoFunBot {
  pub fn launch(config: Config) {
    let mut bot = NoFunBot { config: config.clone() };

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
      Line{command: IRCCmd(cmd), args, prefix: prefix } => match cmd.as_slice() {
        "JOIN" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
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
        }
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
      match self.config.channels.iter().find(|chan| chan.name == dst) {
        Some(channel) => {
          self.moderate(conn, src, channel.clone(), msg)
        },
        None => {
          debug!("Silently ignoring...");
        }
      }
    }
  }

  pub fn moderate(&mut self, conn: &mut Conn, nick: String, channel: &Channel, msg: String) {
      if msg.as_slice() == "hi i'm beasway" {
         conn.privmsg(channel.name.as_bytes(), b"BOOO!");
      }
  }
}
