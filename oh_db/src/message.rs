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

make_error!(ParseError; {
    MissingField => String,
    WrongFieldType => String,
    UnknownType => String
});

// The result of parsing is a Message or an error.
pub type ParseResult = Result<Message, ParseError>;

#[derive(Debug)]
pub enum Message {
    Ping(PingPayload), // ping => pong, version
    CreateChild(CreateChildPayload), // parent_path, name => status
    SubscribeKey(SubscribeKeyPayload), // path, key => status
    //CreateKey(CreateKeyPayload), // path, key => status
    //SetKey(SetKeyPayload), // path, key, value => status
    //GetKey(GetKeyPayload), // path, key => status, value
    //ListKeys(ListKeysPayload), // path => status, [key names]
    //ListChildren(ListChildrenPayload), // path => status, [children names]
}


// ////////////////////////////////////////////////////////////////////////////
// Ping
//
//     A service level ping-pong that carries extra metadata about the
//     service. This lets clients verify that they are connecting to the
//     the right server, supporting the right protocol, etc.
//
//     Request Format:
//       {
//         "type": "Ping",
//         "data": "<whatevs>"
//       }
//
//     Response Format:
//       {
//         "pong": "<same as data>",
//         "protocol_version": Number
//       }
//
//     Errors:
//       <none>
//
#[derive(RustcEncodable)]
pub struct PingResponse {
    pub pong: String,  // The string that the client sent in the |ping| field.
    //pub protocol_version: i32,  // The protcol version.
}

#[derive(Debug)]
pub struct PingPayload {
    pub data: String
}

impl PingPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let data_field = get_field!(message, "data", as_string);
        Ok(Message::Ping(PingPayload{data: data_field.into()}))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// CreateChild
//
//     Add a node to the tree with an empty dictionary.
//     The provided parent path must already exist.
//
//     Request Format:
//       {
//         "type": "CreateChild",
//         "parent_path": "/path/to/parent",
//         "name": "child_name"
//       }
//
//     Response Format:
//       {
//         "status": "Ok | <error>"
//       }
//
//     Errors:
//       NodeAlreadyExists
//       InvalidPathComponent
//
#[derive(Debug)]
pub struct CreateChildPayload {
    pub parent_path: String,
    pub name: String
}

impl CreateChildPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let parent_path_field = get_field!(message, "parent_path", as_string);
        let name_field = get_field!(message, "name", as_string);
        let payload = CreateChildPayload {
            parent_path: parent_path_field.into(),
            name: name_field.into()
        };
        Ok(Message::CreateChild(payload))
    }
}

// ////////////////////////////////////////////////////////////////////////////
// SubscribeKey
//
//     Request to be notified if any of the values at path change.
//     The provided path must already exist.
#[derive(Debug)]
pub struct SubscribeKeyPayload {
    pub path: String,
    pub key: String
}

impl SubscribeKeyPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let key_field = get_field!(message, "key", as_string);
        let payload = SubscribeKeyPayload {
            path: path_field.into(),
            key: key_field.into()
        };
        Ok(Message::SubscribeKey(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// Parse the given message and return the payload.
pub fn parse(data: json::Json) -> ParseResult {
    let message = match data.as_object() {
        Some(a) => a,
        None => return Err(ParseError::WrongFieldType("<root>".into()))
    };

    let type_field = get_field!(message, "type", as_string);
    return match type_field {
        "Ping" => PingPayload::parse(message),
        "CreateChild" => CreateChildPayload::parse(message),
        "SubscribeKey" => SubscribeKeyPayload::parse(message),
        _ => Err(ParseError::UnknownType(type_field.into()))
    };
}

