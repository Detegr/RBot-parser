#[macro_use]
extern crate nom;

use std::borrow::Cow;
use std::str::from_utf8;
use nom::space;
use nom::IResult::*;
use std::str::FromStr;
use std::fmt;

named!(nick_parser <&[u8], &str>, map_res!(chain!(nick: take_until!("!") ~ tag!("!"), ||{nick}), from_utf8));
named!(user_parser <&[u8], &str>, map_res!(chain!(user: take_until!("@") ~ tag!("@"), ||{user}), from_utf8));
named!(word_parser <&[u8], &str>, map_res!(take_until!(" "), from_utf8));
named!(eol <&[u8], &str>, map_res!(take_until_and_consume!("\r"), from_utf8));

#[derive(Debug)]
pub struct ParserError {
    data: String
}
impl std::fmt::Display for ParserError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.data)
    }
}
impl std::error::Error for ParserError {
    fn description(&self) -> &str {
        &self.data
    }
}
impl<'a> From<nom::Err<'a>> for ParserError {
    fn from(e: nom::Err) -> ParserError {
        match e {
            nom::Err::Position(pos, data) => {
                ParserError {
                    data: format!("Error at position {}: '{}'",
                                  pos,
                                  unsafe {std::str::from_utf8_unchecked(data)})
                    }
                }
            err => {
                ParserError {
                    data: format!("Error: {:?}", err)
                }
            }
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum Prefix<'a> {
    User(&'a str, &'a str, &'a str),
    Server(&'a str)
}
impl<'a> fmt::Display for Prefix<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Prefix::User(nick, user, host) => write!(f, "{}!{}@{}", nick, user, host),
            Prefix::Server(serverstr) => write!(f, "{}", serverstr)
        }
    }
}
#[derive(PartialEq, Debug)]
pub enum Command<'a> {
    Named(Cow<'a, str>),
    Numeric(u16)
}
impl<'a> fmt::Display for Command<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Command::Named(ref s) => write!(f, "{}", s),
            Command::Numeric(n) => write!(f, "{}", n)
        }
    }
}

#[derive(Debug)]
pub struct Message<'a> {
    pub prefix: Option<Prefix<'a>>,
    pub command: Command<'a>,
    pub params: Vec<&'a str>
}
impl<'a> Message<'a> {
    pub fn to_whitespace_separated(&self) -> String {
        // TODO: I don't think this ret.push_str() stuff is ideal
        let mut ret = String::new();
        ret.push_str(&self.command.to_string()[..]);
        ret.push_str(&" ");
        match self.prefix {
            Some(ref prefix) => ret.push_str(&prefix.to_string()[..]),
            None => {}
        };
        ret.push_str(&" ");
        ret.push_str(&self.params[..].join(" ")[..]);
        ret
    }
}

impl<'a> fmt::Display for Message<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: I don't think this ret.push_str() stuff is ideal
        let mut ret = match self.prefix {
            Some(ref prefix) => format!(":{} ", prefix),
            None => "".to_string()
        };
        ret.push_str(format!("{} ", self.command).as_ref());
        for param in self.params.iter() {
            // TODO: The output format of this is not 1:1 to the string that was parsed
            ret.push_str(format!("{} ", param).as_ref());
        }
        write!(f, "{}", ret)
    }
}

named!(message_parser <&[u8], Message>,
    chain!(
        parsed_prefix: prefix_parser? ~
        parsed_command: command_parser ~
        parsed_params: map_res!(take_until_and_consume!(":"), from_utf8)? ~
        parsed_trailing: eol,
        || {
            let params = match parsed_params {
                Some(p) => {
                    let _: &str = p; // TODO: This looks stupid. How should this be done?
                    p.split_whitespace()
                        .chain(::std::iter::repeat(parsed_trailing).take(1))
                        .collect()
                },
                None => parsed_trailing.split_whitespace().collect()
            };
            Message {
                prefix: parsed_prefix,
                command: parsed_command,
                params: params
            }
        }
    )
);

named!(command_parser <&[u8], Command>,
    chain!(
        cmd: word_parser,
        || {
            match FromStr::from_str(cmd) {
                Ok(numericcmd) => Command::Numeric(numericcmd),
                Err(_) => Command::Named(cmd.into())
            }
        }
    )
);

named!(prefix_parser <&[u8], Prefix>,
    chain!(
        tag!(":") ~
        prefix: word_parser ~
        space,
        || {
            match host_parser(prefix.as_bytes()) {
                Done(_, (nick, user, host)) => Prefix::User(nick, user, host),
                _ => Prefix::Server(prefix)
            }
        }
    )
);
named!(host_parser <&[u8], (&str, &str, &str)>,
    chain!(
       nick: nick_parser ~
       user: user_parser ~
       host: word_parser ,
       ||{(nick, user, host)}
    )
);

pub fn parse_message(input: &str) -> Result<Message, ParserError> {
    match message_parser(input.as_bytes()) {
        Done(_, msg) => Ok(msg),
        Incomplete(i) => Err(ParserError {
            data: format!("Incomplete: {:?}", i)
        }),
        Error(e) => Err(From::from(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::IResult::*;
    #[test]
    fn test_parsing_host() {
        match super::host_parser(b"user!host@example.com ") {
            Done(_, (nick, user, host)) => {
                assert_eq!(nick, "user");
                assert_eq!(user, "host");
                assert_eq!(host, "example.com");
            },
            Incomplete(i) => panic!(format!("Incomplete: {:?}", i)),
            _ => panic!("Error while parsing host")
        }
    }
    #[test]
    fn test_parsing_line() {
        match super::message_parser(b"NOTICE AUTH :*** Looking up your hostname\r") {
            Done(_, msg) => {
                assert_eq!(msg.prefix, None);
                assert_eq!(msg.command, Command::Named("NOTICE".into()));
                assert_eq!(msg.params, vec!["AUTH", "*** Looking up your hostname"]);
            },
            Incomplete(i) => panic!(format!("Incomplete: {:?}", i)),
            _ => panic!("Error while parsing auth message")
        }
    }
    #[test]
    fn test_parsing_line_without_trailing() {
        match super::message_parser(b":port80a.se.quakenet.org 004 RustBot port80a.se.quakenet.org u2.10.12.10+snircd(1.3.4a) dioswkgxRXInP biklmnopstvrDcCNuMT bklov\r\n") {
            Done(_, msg) => {
                assert_eq!(msg.prefix, Some(Prefix::Server("port80a.se.quakenet.org")));
                assert_eq!(msg.command, Command::Numeric(4));
                assert_eq!(msg.params, vec!["RustBot", "port80a.se.quakenet.org", "u2.10.12.10+snircd(1.3.4a)", "dioswkgxRXInP", "biklmnopstvrDcCNuMT", "bklov"]);
            },
            Incomplete(i) => panic!(format!("Incomplete: {:?}", i)),
            _ => panic!("Error while parsing a message without trailing stuff")
        }
    }
    #[test]
    fn test_parsing_prefix() {
        match super::prefix_parser(b":this.represents.a.server.prefix ") {
            Done(left, Prefix::Server(server)) => {
                assert_eq!(server, "this.represents.a.server.prefix");
                assert_eq!(left.len(), 0);
            },
            Incomplete(i) => panic!(format!("Incomplete: {:?}", i)),
            _ => panic!("Error while parsing prefix")
        }
    }
    #[test]
    fn test_parsing_message_using_parse_message() {
        let msg = "NOTICE AUTH :*** Looking up your hostname\r\nNOTICE AUTH :*** Checking Ident\r\nNOTICE AUTH :*** Found your hostname\r\n";
        for m in msg.split("\n") {
            if m.len() <= 1 { continue; } // TODO: Better way to split?
            match parse_message(m) {
                Ok(_) => {},
                Err(_) => panic!("Could not parse line {:?} from message.", m)
            };
        }
    }
    #[test]
    fn test_whitespace_separated() {
        let parsed = parse_message(":user!host@example.com PRIVMSG #channel :message\r\n").unwrap();
        assert_eq!(parsed.to_whitespace_separated(), "PRIVMSG user!host@example.com #channel message");
    }

    #[test]
    fn test_inline_host() {
        parse_message(":server.example.com 333 RustBot #channel user!host@example.com 123456789\r\n").unwrap();
    }
}
