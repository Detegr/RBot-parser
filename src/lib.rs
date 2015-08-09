#[macro_use]
extern crate nom;

use std::str::from_utf8;
use nom::space;
use nom::IResult::*;
use std::str::FromStr;
use std::fmt;

named!(nick_parser <&[u8], &str>, map_res!(chain!(nick: take_until!("!") ~ tag!("!"), ||{nick}), from_utf8));
named!(user_parser <&[u8], &str>, map_res!(chain!(user: take_until!("@") ~ tag!("@"), ||{user}), from_utf8));
named!(word_parser <&[u8], &str>, map_res!(take_until!(" "), from_utf8));
named!(eol <&[u8], &str>, map_res!(take_until_and_consume!("\r"), from_utf8));

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
    Named(&'a str),
    Numeric(u16)
}
impl<'a> fmt::Display for Command<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Command::Named(s) => write!(f, "{}", s),
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
        parsed_command: command_parser   ~
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
                Err(_) => Command::Named(cmd)
            }
        }
    )
);

named!(prefix_parser <&[u8], Prefix>,
    chain!(
        tag!(":") ~
        hostprefix: host_parser?   ~
        serverprefix: word_parser? ~
        space                      ,
        || {
            match hostprefix {
                Some((nick, user, host)) => Prefix::User(nick, user, host),
                None => Prefix::Server(serverprefix.unwrap())
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

pub fn parse_message(input: &str) -> Result<Message, String>
{
    match message_parser(input.as_bytes()) {
        Done(_, msg) => Ok(msg),
        Incomplete(i) => Err(format!("Incomplete: {:?}", i)),
        Error(i) => Err(format!("Error: {:?}", i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::IResult::*;
    #[test]
    fn test_parsing_user() {
        match super::prefix_parser(b":user!host@example.com ") {
            Done(left, Prefix::User(nick, user, host)) => {
                assert_eq!(nick, "user");
                assert_eq!(user, "host");
                assert_eq!(host, "example.com");
                assert_eq!(left.len(), 0);
            },
            Incomplete(i) => panic!(format!("Incomplete: {:?}", i)),
            _ => panic!("Error while parsing prefix")
        }
    }
    #[test]
    fn test_parsing_line() {
        match super::message_parser(b"NOTICE AUTH :*** Looking up your hostname\r") {
            Done(_, msg) => {
                assert_eq!(msg.prefix, None);
                assert_eq!(msg.command, Command::Named("NOTICE"));
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
}
