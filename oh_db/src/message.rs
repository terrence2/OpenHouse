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
    IdOutOfRange => u64,
    MissingField => String,
    WrongFieldType => String,
    UnknownType => String
});

// The result of parsing is a Message or an error.
pub type ParseResult = Result<Message, ParseError>;

/// The largest integer which is uniquely representable by
/// an f64/double/Number. This is important since we want to
/// safely round-trip identifiers through JSON.
const MAX_SAFE_ID: u64 = 9007199254740991;

#[derive(Debug)]
pub enum Message {
    // Establish that the channel works.
    Ping(u64, PingPayload), // ping => pong, version

    // Manage Tree Shape.
    CreateChild(u64, CreateChildPayload), // parent_path, name => status
    RemoveChild(u64, RemoveChildPayload), // path => status
    ListChildren(u64, ListChildrenPayload), // path => status, [children names]
    SubscribeLayout(u64, SubscribeLayoutPayload), // path => status

    // Manage Data Content.
    //CreateKey(u64, CreateKeyPayload), // path, key => status
    //SetKey(u64, SetKeyPayload), // path, key, value => status
    //GetKey(u64, GetKeyPayload), // path, key => status, value
    //ListKeys(u64, ListKeysPayload), // path => status, [key names]
    SubscribeKey(u64, SubscribeKeyPayload), // path, key => status
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
//         "message_id": Number,
//         "type": "Ping",
//         "data": "<whatevs>"
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "pong": "<same as data>",
//         "protocol_version": Number
//       }
//
//     Errors:
//       <none>
//
#[derive(Debug)]
pub struct PingPayload {
    pub data: String
}

impl PingPayload {
    fn parse(message_id: u64, message: &json::Object) -> ParseResult {
        let data_field = get_field!(message, "data", as_string);
        Ok(Message::Ping(message_id, PingPayload{data: data_field.into()}))
    }
}

#[derive(RustcEncodable)]
pub struct PingResponse {
    pub message_id: u64,
    pub pong: String,  // The string that the client sent in the |ping| field.
    //pub protocol_version: i32,  // The protcol version.
}


// ////////////////////////////////////////////////////////////////////////////
// CreateChild
//
//     Add a node to the tree with an empty dictionary.
//     The provided parent path must already exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "CreateChild",
//         "parent_path": "/path/to/parent",
//         "name": "child_name"
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>"
//         ["context": "information about error"]
//       }
//
//     Errors:
//       InvalidPathComponent
//       MalformedPath
//       NoSuchNode
//       NodeAlreadyExists
//
#[derive(Debug)]
pub struct CreateChildPayload {
    pub parent_path: String,
    pub name: String
}

impl CreateChildPayload {
    fn parse(message_id: u64, message: &json::Object) -> ParseResult {
        let parent_path_field = get_field!(message, "parent_path", as_string);
        let name_field = get_field!(message, "name", as_string);
        let payload = CreateChildPayload {
            parent_path: parent_path_field.into(),
            name: name_field.into()
        };
        Ok(Message::CreateChild(message_id, payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// RemoveChild
//
//     Remove the node at the given path with |name| from the tree.
//     The provided parent path must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "RemoveChild",
//         "parent_path": "/path/to/parent",
//         "name": "child_name"
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>"
//         ["context": "information about error"]
//       }
//
//     Errors:
//       InvalidPathComponent
//       MalformedPath
//       NoSuchNode
//       NodeContainsChildren
//       NodeContainsKeys
//
#[derive(Debug)]
pub struct RemoveChildPayload {
    pub parent_path: String,
    pub name: String
}

impl RemoveChildPayload {
    fn parse(message_id: u64, message: &json::Object) -> ParseResult {
        let parent_path_field = get_field!(message, "parent_path", as_string);
        let name_field = get_field!(message, "name", as_string);
        let payload = RemoveChildPayload {
            parent_path: parent_path_field.into(),
            name: name_field.into()
        };
        Ok(Message::RemoveChild(message_id, payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// ListChildren
//
//     Return a list of direct children of the given path.
//     The given path must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "ListChildren",
//         "path": "/path/to/list",
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>",
//         ["context": "information about error" ||
//          "children": ["list", "of", ... "children"]]
//       }
//
//     Errors:
//       MalformedPath
//       NoSuchNode
//
#[derive(Debug)]
pub struct ListChildrenPayload {
    pub path: String,
}

impl ListChildrenPayload {
    fn parse(message_id: u64, message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let payload = ListChildrenPayload {
            path: path_field.into(),
        };
        Ok(Message::ListChildren(message_id, payload))
    }
}

#[derive(RustcEncodable)]
pub struct ListChildrenResponse {
    pub message_id: u64,
    pub status: String,
    pub children: Vec<String>
}


// ////////////////////////////////////////////////////////////////////////////
// SubscribeLayout
//
//     Register to receive messages whenever the children of the given path
//     change. The provided path must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "SubscribeLayout",
//         "path": "/path/to/node"
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>",
//         ["context": "information about error" ||
//          "subscription_id": Number]
//       }
//
//     Subscription Message Format:
//       {
//         "subscription_id": Number,
//         "path": "/path/to/node",
//         "event": "Create" || "Remove",
//         "name": "NodeName"
//       }
//
//     Errors:
//       MalformedPath
//       NoSuchNode
//
#[derive(Debug)]
pub struct SubscribeLayoutPayload {
    pub path: String,
}

impl SubscribeLayoutPayload {
    fn parse(message_id: u64, message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let payload = SubscribeLayoutPayload {
            path: path_field.into()
        };
        Ok(Message::SubscribeLayout(message_id, payload))
    }
}

#[derive(RustcEncodable)]
pub struct SubscribeLayoutResponse {
    pub message_id: u64,
    pub status: String,
    pub subscription_id: u64
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
    fn parse(message_id: u64, message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let key_field = get_field!(message, "key", as_string);
        let payload = SubscribeKeyPayload {
            path: path_field.into(),
            key: key_field.into()
        };
        Ok(Message::SubscribeKey(message_id, payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// Parse the given message and return the payload.
pub fn parse_message(data: json::Json) -> ParseResult {
    let message = match data.as_object() {
        Some(a) => a,
        None => return Err(ParseError::WrongFieldType("<root>".into()))
    };

    let message_id = get_field!(message, "message_id", as_u64);
    if message_id == 0 || message_id >= MAX_SAFE_ID {
        return Err(ParseError::IdOutOfRange(message_id));
    }

    let type_field = get_field!(message, "type", as_string);
    return match type_field {
        "Ping" => PingPayload::parse(message_id, message),
        "CreateChild" => CreateChildPayload::parse(message_id, message),
        "RemoveChild" => RemoveChildPayload::parse(message_id, message),
        "ListChildren" => ListChildrenPayload::parse(message_id, message),
        "SubscribeLayout" => SubscribeLayoutPayload::parse(message_id, message),
        "SubscribeKey" => SubscribeKeyPayload::parse(message_id, message),
        _ => Err(ParseError::UnknownType(type_field.into()))
    };
}

