use irc::conn::Conn;
use Channel;

pub fn score(msg: &String) -> u32 {
  debug!("Scoring message: {}", msg);
  // todo: these could be cached somewhere
  let regexes = [
    regex!(r"(?i)k[a@e3][e3p]p[ao@]"), // kappas
    regex!(r"(?i)doge"), // nice meme
    regex!(r"(?i)lenny[ ]?face"),
    regex!(r"BibleThump"),
    regex!(r"blis donate"),
    regex!(r"(?i)or riot"),
    regex!(r"(?i)donger"),
    regex!(r"(?i)ez skins ez l[iy]fe"),
    regex!(r"(?i)clutch or kick")
  ];
  for re in regexes.iter() {
    if re.is_match(msg.as_slice()) {
      return 3;
    }
  }

  0
}

