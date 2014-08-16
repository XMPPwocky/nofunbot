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
    nick: "NoFunBot",
    server: "irc.quakenet.org",
    port: 6667
  });
}

pub struct Config<'a> {
  nick: &'a str,
  server: &'a str,
  port: u16
}

pub struct NoFunBot;

impl NoFunBot {
  pub fn launch(config: Config) {
    let mut ircopts = irc::conn::Options::new(config.server, config.port);
    ircopts.nick = config.nick.as_slice();

    let mut bot = NoFunBot;

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
        println!("Logged in");
        // we've logged in
        conn.join(bytes!("#nofunbot"), [])
      }
      Line{command: IRCCmd(cmd), args, prefix: prefix } => match cmd.as_slice() {
        "JOIN" if prefix.is_some() => {
          let prefix = prefix.unwrap();
          if prefix.nick() != conn.me().nick() {
            return;
          }
          if args.is_empty() {
            let line = Line{command: IRCCmd("JOIN".into_maybe_owned()),
            args: args, prefix: Some(prefix)};
            println!("ERROR: Invalid JOIN message received: {}", line);
            return;
          }
          let chan = args.move_iter().next().unwrap();
          conn.privmsg(chan.as_slice(), bytes!("Hello"));
          let chan = String::from_utf8_lossy(chan.as_slice());
          println!("JOINED: {}", chan);
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
              print!("ERROR: Unexpected {} line: ", cmd);
              let line = Line{command: IRCCmd(cmd), args: args, prefix: prefix};
              println!("{}", line);
              return;
            }
          };
          let dsts = String::from_utf8_lossy(dst.as_slice());
          let srcs = String::from_utf8_lossy(src.as_slice());
          let msgs = String::from_utf8_lossy(msg.as_slice());
          println!("<-- {}({}) {}: {}", cmd, dsts, srcs, msgs);
          // handle_privmsg(conn, msg.as_slice(), src, dst.as_slice())
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
            println!("ERROR: Unexpected ACTION line: {}", line);
            return;
          }
        };
        let dst = String::from_utf8_lossy(dst.as_slice());
        let src = String::from_utf8_lossy(src.as_slice());
        let msg = String::from_utf8_lossy(msg.as_slice());
        println!("<-- PRIVMSG({}) {} {}", dst, src, msg);
      }
      _ => ()
    }
  }
}
