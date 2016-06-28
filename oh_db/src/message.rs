// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use rustc_serialize::{json, Encoder, Encodable};
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
    UnknownMessageType => String,
    UnknownNodeType => String,
    WrongFieldType => String
});

// The result of parsing is a Message or an error.
pub type ParseResult = Result<Message, ParseError>;

// Produce a "new type" for u64 representing a uid.
macro_rules! make_identifier {
    ($name:ident) => {
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct $name(u64);
        impl $name {
            pub fn from_u64(ident: u64) -> $name {
                $name(ident)
            }
        }
        impl Clone for $name {
            fn clone(&self) -> $name {
                let $name(ident) = *self;
                return $name(ident);
            }
        }
        impl Copy for $name {}
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let $name(ident) = *self;
                write!(f, "{}", ident)
            }
        }
        impl Encodable for $name {
            fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
                let $name(ident) = *self;
                s.emit_u64(ident)
            }
        }
    };
}

make_identifier!(SubscriptionId);
make_identifier!(MessageId);

/// The largest integer which is uniquely representable by
/// an f64/double/Number. This is important since we want to
/// safely round-trip identifiers through JSON.
const MAX_SAFE_ID: u64 = 9007199254740991;

#[derive(Debug)]
pub enum Message {
    // Establish that the channel works.
    Ping(PingPayload), // ping => pong, version

    // File/Directory management.
    CreateNode(CreateNodePayload), // parent_path, name => status
    ListDirectory(ListDirectoryPayload), // path => status, [children names]
    SetFileContent(SetFileContentPayload), // path, data => status
    GetFileContent(GetFileContentPayload), // path => status, [data]
    RemoveNode(RemoveNodePayload), // path => status

    // Subscription management.
    Subscribe(SubscribePayload), // path => status
    Unsubscribe(UnsubscribePayload), // uid => status
}


// ////////////////////////////////////////////////////////////////////////////
// Ping
//
//     A service level ping-pong that carries extra metadata about the service.
//     This lets clients verify that they are connecting to the the right
//     server, supporting the right protocol, etc.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "Ping",
//         "message_payload" {
//           "data": "<whatevs>"
//         }
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
    fn parse(payload: &json::Object) -> ParseResult {
        let data_field = get_field!(payload, "data", as_string);
        Ok(Message::Ping(PingPayload{data: data_field.into()}))
    }
}

#[derive(RustcEncodable)]
pub struct PingResponse {
    pub message_id: MessageId,
    pub pong: String,  // The string that the client sent in the |ping| field.
    //pub protocol_version: i32,  // The protcol version.
}


// ////////////////////////////////////////////////////////////////////////////
// CreateNode
//
//     Add a node to the tree with an empty dictionary. The provided parent
//     path must already exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "CreateNode",
//         "message_payload": {
//           "parent_path": "/path/to/parent",
//           "node_type": "File|Directory",
//           "name": "child_name"
//         }
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
//       NotDirectory
//
#[derive(Debug)]
pub enum NodeType { File, Directory }
impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NodeType::File => write!(f, "NodeType::File"),
            NodeType::Directory => write!(f, "NodeType::Directory")
        }
    }
}
impl NodeType {
    fn parse(type_str: &str) -> Result<NodeType, ParseError> {
        match type_str {
            "File" => Ok(NodeType::File),
            "Directory" => Ok(NodeType::Directory),
            _ => Err(ParseError::UnknownNodeType(type_str.into()))
        }
    }
}

#[derive(Debug)]
pub struct CreateNodePayload {
    pub node_type: NodeType,
    pub parent_path: String,
    pub name: String
}

impl CreateNodePayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let parent_path_field = get_field!(payload, "parent_path", as_string);
        let type_field = get_field!(payload, "type", as_string);
        let name_field = get_field!(payload, "name", as_string);
        let payload = CreateNodePayload {
            parent_path: parent_path_field.into(),
            node_type: try!(NodeType::parse(type_field)),
            name: name_field.into()
        };
        Ok(Message::CreateNode(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// ListDirectory
//
//     Return a list of direct children of the given path. The given path must
//     exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "ListDirectory",
//         "message_payload": {
//           "path": "/path/to/list",
//         }
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
//       NotDirectory
//
#[derive(Debug)]
pub struct ListDirectoryPayload {
    pub path: String,
}

impl ListDirectoryPayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let path_field = get_field!(payload, "path", as_string);
        let payload = ListDirectoryPayload {
            path: path_field.into(),
        };
        Ok(Message::ListDirectory(payload))
    }
}

#[derive(RustcEncodable)]
pub struct ListDirectoryResponse {
    pub message_id: MessageId,
    pub status: String,
    pub children: Vec<String>
}


// ////////////////////////////////////////////////////////////////////////////
// GetFileContent
//
//     Return a the given files contents. The given file must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "GetFileContent",
//         "message_payload": {
//           "path": "/path/to/get",
//         }
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>",
//         ["context": "information about error" ||
//          "data": "base64 encoded string"]
//       }
//
//     Errors:
//       MalformedPath
//       NoSuchNode
//       NotFile
//
#[derive(Debug)]
pub struct GetFileContentPayload {
    pub path: String,
}

impl GetFileContentPayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let path_field = get_field!(payload, "path", as_string);
        let payload = GetFileContentPayload {
            path: path_field.into(),
        };
        Ok(Message::GetFileContent(payload))
    }
}

#[derive(RustcEncodable)]
pub struct GetFileContentResponse {
    pub message_id: MessageId,
    pub status: String,
    pub data: String
}


// ////////////////////////////////////////////////////////////////////////////
// SetFileContent
//
//     Return a the given files contents. The given file must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "SetFileContent",
//         "message_payload": {
//           "path": "/path/to/get",
//           "data": "utf-8 encoded string"
//         }
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>",
//         ["context": "information about error"]
//       }
//
//     Errors:
//       MalformedPath
//       NoSuchNode
//       NotFile
//
#[derive(Debug)]
pub struct SetFileContentPayload {
    pub path: String,
    pub data: String
}

impl SetFileContentPayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let path_field = get_field!(payload, "path", as_string);
        let data_field = get_field!(payload, "data", as_string);
        let payload = SetFileContentPayload {
            path: path_field.into(),
            data: data_field.into(),
        };
        Ok(Message::SetFileContent(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// RemoveNode
//
//     Remove the node at the given path with |name| from the tree. The
//     provided parent path must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "RemoveNode",
//         "message_payload": {
//           "parent_path": "/path/to/parent",
//           "name": "name"
//         }
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
//       NodeContainsSubscriptions
//       NodeContainsData
//
#[derive(Debug)]
pub struct RemoveNodePayload {
    pub parent_path: String,
    pub name: String
}

impl RemoveNodePayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let parent_path_field = get_field!(payload, "parent_path", as_string);
        let name_field = get_field!(payload, "name", as_string);
        let payload = RemoveNodePayload {
            parent_path: parent_path_field.into(),
            name: name_field.into(),
        };
        Ok(Message::RemoveNode(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// Subscribe
//
//     Register to receive messages whenever the children of the given path
//     change. The provided path must exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "Subscribe",
//         "message_payload": {
//           "path": "/path/to/node"
//         }
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
//         "layout_subscription_id": Number,
//         "path": "/path/to/node",
//         "event": "Create" || "Remove" || "Changed",
//         "context": "description of what changed"
//       }
//
//     Errors:
//       MalformedPath
//       NoSuchNode
//
#[derive(Debug)]
pub struct SubscribePayload {
    pub path: String,
}

impl SubscribePayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let path_field = get_field!(payload, "path", as_string);
        let payload = SubscribePayload {
            path: path_field.into()
        };
        Ok(Message::Subscribe(payload))
    }
}

#[derive(RustcEncodable)]
pub struct SubscribeResponse {
    pub message_id: MessageId,
    pub status: String,
    pub subscription_id: SubscriptionId
}

#[derive(RustcEncodable)]
pub struct SubscriptionMessage {
    pub subscription_id: SubscriptionId,
    pub path: String,
    pub event: String,
    pub context: String
}


// ////////////////////////////////////////////////////////////////////////////
// Unsubscribe
//
//     Remove an existing subscription. The provided subscription must
//     exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "message_type": "Unsubscribe",
//         "message_payload": {
//           "subscription_id": Number
//         }
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>",
//         ["context": "information about error"]
//       }
//
//     Errors:
//       NoSuchSubscription
//
#[derive(Debug)]
pub struct UnsubscribePayload {
    pub subscription_id: SubscriptionId,
}

impl UnsubscribePayload {
    fn parse(payload: &json::Object) -> ParseResult {
        let sid_field = get_field!(payload, "subscription_id", as_u64);
        let payload = UnsubscribePayload {
            subscription_id: SubscriptionId::from_u64(sid_field.into()),
        };
        Ok(Message::Unsubscribe(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// Parse the id out of the given message and return it to the caller.
pub fn parse_message_id(data: &json::Json) -> Result<MessageId, ParseError> {
    let message = match data.as_object() {
        Some(a) => a,
        None => return Err(ParseError::WrongFieldType("<root>".into()))
    };

    let message_id = get_field!(message, "message_id", as_u64);
    if message_id == 0 || message_id >= MAX_SAFE_ID {
        return Err(ParseError::IdOutOfRange(message_id));
    }

    return Ok(MessageId::from_u64(message_id));
}


// ////////////////////////////////////////////////////////////////////////////
// Parse the given message and return the payload.
pub fn parse_message(data: &json::Json) -> ParseResult {
    let message = match data.as_object() {
        Some(a) => a,
        None => return Err(ParseError::WrongFieldType("<root>".into()))
    };

    let payload_field = get_field!(message, "message_payload", as_object);

    let type_field = get_field!(message, "message_type", as_string);
    return match type_field {
        "Ping" => PingPayload::parse(payload_field.into()),
        "CreateNode" => CreateNodePayload::parse(payload_field.into()),
        "ListDirectory" => ListDirectoryPayload::parse(payload_field.into()),
        "GetFileContent" => GetFileContentPayload::parse(payload_field.into()),
        "SetFileContent" => SetFileContentPayload::parse(payload_field.into()),
        "RemoveNode" => RemoveNodePayload::parse(payload_field.into()),
        "Subscribe" => SubscribePayload::parse(payload_field.into()),
        "Unsubscribe" => UnsubscribePayload::parse(payload_field.into()),
        _ => Err(ParseError::UnknownMessageType(type_field.into()))
    };
}

