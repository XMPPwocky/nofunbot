use irc::conn::Conn;
use flate;
use chrono;

use {RulesCheckResult, Infraction, RulesOK};
pub fn check(msg: &str, state: &mut ::UserState) -> ::RulesCheckResult {
  let time_since_last = chrono::UTC::now() - state.last_message_time;
  debug!("Scoring message: {}", msg);
  // todo: these could be cached somewhere
  let regexes = [
    regex!(r"(?i)^k[a@e3][e3p]p[ao@]$"), // kappas
    regex!(r"(?i)^doge$"), // nice meme
    regex!(r"(?i)lenny[ ]?face"),
    regex!(r"BibleThump"),
    regex!(r"blis donate"),
    regex!(r"(?i)or riot"),
    regex!(r"(?i)donger"),
    regex!(r"(?i)ez skins ez l[iy]fe"),
    regex!(r"(?i)clutch or kick"),
//    regex!(r"(organner)|(aimware)|(aimjunkies)"),
    regex!(r"pl[sz] no .*erino"), // pls no spammerino
  ];
  for re in regexes.iter() {
    if re.is_match(msg) {
      return Infraction("This isn't Twitch chat.")
    }
  }

  // single word messages are bad
  if regex!(r"\s+[^$]").find_iter(msg.trim_chars(' ')).count() == 0 && time_since_last.num_seconds() <= 3 {
    state.simple_msg_count += 1;

    if state.simple_msg_count >= 3 {
      state.simple_msg_count = 0;
      return Infraction("Please use longer sentences, instead of many short ones")
    }
  } else {
    state.simple_msg_count = 0;
  }

  if msg.len() > 6 && msg == state.last_message.as_slice()
    && (chrono::UTC::now() - state.last_message_time).num_seconds() < 2 {
      return Infraction("Is there an echo in here?")
    }

  if complexity_test(msg) {
    Infraction("Stop spamming.")
  } else { 
    RulesOK
  }
}

pub fn complexity_test(msg: &str) -> bool {
  // uses compression ratio w/ zlib as a proxy for complexity.
  if msg.len() < 16 {
    return false;
  }
  let msg_bytes = msg.as_bytes();
  match flate::deflate_bytes(msg_bytes) {
    Some(compressed) => {
      let ratio = msg_bytes.len() as f32 / compressed.len() as f32;
      debug!("Compression ratio of {} is {}", msg, ratio);

      let threshold = 2.0 + (0.15 * msg_bytes.len() as f32 / 10.0);
      ratio > threshold
    },
    None => { warn!("No compression?"); false }
  }
}
