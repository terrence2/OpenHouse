// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use rustc_serialize::json;
use std::error::Error;
use std::fmt;

// Extract $field and call $type_conv on it. Return a ParseError if the
// field does not exist or the call fails.
macro_rules! get_field {
    ($obj:ident, $field:expr, $type_conv:ident) => {
        match $obj.get($field) {
            Some(a) => { match a.$type_conv() {
                Some(b) => b,
                None => return Err(ParseError::WrongFieldType($field.into()))
            }},
            None => return Err(ParseError::MissingField($field.into()))
        }
    };
}

#[derive(Debug)]
pub enum ParseError {
    MissingField(String),
    WrongFieldType(String),
    UnknownType(String)
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::MissingField(ref err) => write!(f, "Missing field {}", err),
            ParseError::WrongFieldType(ref err) => write!(f, "Wrong field type on {}", err),
            ParseError::UnknownType(ref err) => write!(f, "The message type {} is unknown", err),
        }
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::MissingField(_) => "missing field",
            ParseError::WrongFieldType(_) => "wrong field type",
            ParseError::UnknownType(_) => "unknown message type",
        }
    }
}

// The result of parsing is a Message or an error.
pub type ParseResult = Result<Message, ParseError>;

#[derive(Debug)]
pub enum Message {
    Ping(PingPayload),
    Subscribe(SubscribeMessage),
}


// Implement ping.
#[derive(Debug)]
pub struct PingPayload {
    pub data: String
}

impl PingPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let ping_field = get_field!(message, "ping", as_string);
        Ok(Message::Ping(PingPayload{data: ping_field.into()}))
    }
}

#[derive(RustcEncodable)]
pub struct PingResponse {
    pub pong: String
}


// Implement subscribe.
#[derive(Debug)]
pub struct SubscribeMessage {
    target: String
}

impl SubscribeMessage {
    fn parse(message: &json::Object) -> ParseResult {
        let target_field = get_field!(message, "target", as_string);
        Ok(Message::Subscribe(SubscribeMessage{target: target_field.into()}))
    }
}

pub fn parse(data: json::Json) -> ParseResult {
    let message = match data.as_object() {
        Some(a) => a,
        None => return Err(ParseError::WrongFieldType("<root>".into()))
    };

    let type_field = get_field!(message, "type", as_string);
    return match type_field {
        "ping" => PingPayload::parse(message),
        "subscribe" => SubscribeMessage::parse(message),
        _ => Err(ParseError::UnknownType(type_field.into()))
    };
}

