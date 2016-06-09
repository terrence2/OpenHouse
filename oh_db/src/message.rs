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
    WrongFieldType => String,
    UnknownType => String
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

make_identifier!(KeysSubscriptionId);
make_identifier!(LayoutSubscriptionId);
make_identifier!(MessageId);

/// The largest integer which is uniquely representable by
/// an f64/double/Number. This is important since we want to
/// safely round-trip identifiers through JSON.
const MAX_SAFE_ID: u64 = 9007199254740991;

#[derive(Debug)]
pub enum Message {
    // Establish that the channel works.
    Ping(PingPayload), // ping => pong, version

    // Manage Tree Shape.
    CreateChild(CreateChildPayload), // parent_path, name => status
    RemoveChild(RemoveChildPayload), // path => status
    ListChildren(ListChildrenPayload), // path => status, [children names]
    SubscribeLayout(SubscribeLayoutPayload), // path => status
    UnsubscribeLayout(UnsubscribeLayoutPayload), // uid => status

    // Manage Data Content.
    //CreateKey(CreateKeyPayload), // path, key => status
    //RemoveKey(RemoveKeyPayload), // path, key => status
    //SetKey(SetKeyPayload), // path, key, value => status
    //GetKey(GetKeyPayload), // path, key => status, value
    //ListKeys(ListKeysPayload), // path => status, [key names]
    SubscribeKeys(SubscribeKeysPayload), // path, key => status
    UnsubscribeKeys(UnsubscribeKeysPayload), // uid => status
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
    fn parse(message: &json::Object) -> ParseResult {
        let data_field = get_field!(message, "data", as_string);
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
// CreateChild
//
//     Add a node to the tree with an empty dictionary. The provided parent
//     path must already exist.
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
// RemoveChild
//
//     Remove the node at the given path with |name| from the tree. The
//     provided parent path must exist.
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
//       NodeContainsSubscriptions
//       NodeContainsData
//
#[derive(Debug)]
pub struct RemoveChildPayload {
    pub parent_path: String,
    pub name: String
}

impl RemoveChildPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let parent_path_field = get_field!(message, "parent_path", as_string);
        let name_field = get_field!(message, "name", as_string);
        let payload = RemoveChildPayload {
            parent_path: parent_path_field.into(),
            name: name_field.into()
        };
        Ok(Message::RemoveChild(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// ListChildren
//
//     Return a list of direct children of the given path. The given path must
//     exist.
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
    fn parse(message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let payload = ListChildrenPayload {
            path: path_field.into(),
        };
        Ok(Message::ListChildren(payload))
    }
}

#[derive(RustcEncodable)]
pub struct ListChildrenResponse {
    pub message_id: MessageId,
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
//          "layout_subscription_id": Number]
//       }
//
//     Subscription Message Format:
//       {
//         "layout_subscription_id": Number,
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
    fn parse(message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let payload = SubscribeLayoutPayload {
            path: path_field.into()
        };
        Ok(Message::SubscribeLayout(payload))
    }
}

#[derive(RustcEncodable)]
pub struct SubscribeLayoutResponse {
    pub message_id: MessageId,
    pub status: String,
    pub layout_subscription_id: LayoutSubscriptionId
}

#[derive(RustcEncodable)]
pub struct SubscribeLayoutMessage {
    pub layout_subscription_id: LayoutSubscriptionId,
    pub path: String,
    pub event: String,
    pub name: String
}

// ////////////////////////////////////////////////////////////////////////////
// UnsubscribeLayout
//
//     Remove an existing layout subscription. The provided subscription must
//     exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "UnsubscribeLayout",
//         "layout_subscription_id": Number
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
//       NoSuchLayoutSubscription
//
#[derive(Debug)]
pub struct UnsubscribeLayoutPayload {
    pub layout_subscription_id: LayoutSubscriptionId,
}

impl UnsubscribeLayoutPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let layout_sid_field = get_field!(message, "layout_subscription_id", as_u64);
        let payload = UnsubscribeLayoutPayload {
            layout_subscription_id: LayoutSubscriptionId::from_u64(layout_sid_field.into()),
        };
        Ok(Message::UnsubscribeLayout(payload))
    }
}


// ////////////////////////////////////////////////////////////////////////////
// CreateKey
//
//     Add a key to the given path. The provided path must already exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "CreateKey",
//         "path": "/path/to/node",
//         "name": "key_name"
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

// ////////////////////////////////////////////////////////////////////////////
// SubscribeKeys
//
//     Request to be notified if the set of keys at path change. The data
//     provided is the key that was added or removed. To listen for changes to
//     the data stored in a key, use SubscribeData. The provided path must
//     already exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "SubscribeKeys",
//         "path": "/path/to/node"
//       }
//
//     Response Format:
//       {
//         "message_id": Number,
//         "status": "Ok | <error>",
//         ["context": "information about error" ||
//          "keys_subscription_id": Number]
//       }
//
//     Subscription Message Format:
//       {
//         "keys_subscription_id": Number,
//         "path": "/path/to/node",
//         "event": "Create" || "Remove",
//         "name": "NodeName"
//       }
//
//     Errors:
//       MalformedPath
//       NoSuchNode
#[derive(Debug)]
pub struct SubscribeKeysPayload {
    pub path: String
}

impl SubscribeKeysPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let path_field = get_field!(message, "path", as_string);
        let payload = SubscribeKeysPayload {
            path: path_field.into()
        };
        Ok(Message::SubscribeKeys(payload))
    }
}

#[derive(RustcEncodable)]
pub struct SubscribeKeysResponse {
    pub message_id: MessageId,
    pub status: String,
    pub keys_subscription_id: KeysSubscriptionId
}

#[derive(RustcEncodable)]
pub struct SubscribeKeysMessage {
    pub keys_subscription_id: KeysSubscriptionId,
    pub path: String,
    pub event: String,
    pub name: String
}


// ////////////////////////////////////////////////////////////////////////////
// UnsubscribeKeys
//
//     Remove an existing keys subscription. The provided subscription must
//     exist.
//
//     Request Format:
//       {
//         "message_id": Number,
//         "type": "UnsubscribeKeys",
//         "keys_subscription_id": Number
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
//       NoSuchKeysSubscription
//
#[derive(Debug)]
pub struct UnsubscribeKeysPayload {
    pub keys_subscription_id: KeysSubscriptionId,
}

impl UnsubscribeKeysPayload {
    fn parse(message: &json::Object) -> ParseResult {
        let keys_sid_field = get_field!(message, "keys_subscription_id", as_u64);
        let payload = UnsubscribeKeysPayload {
            keys_subscription_id: KeysSubscriptionId::from_u64(keys_sid_field.into()),
        };
        Ok(Message::UnsubscribeKeys(payload))
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

    let type_field = get_field!(message, "type", as_string);
    return match type_field {
        "Ping" => PingPayload::parse(message),
        "CreateChild" => CreateChildPayload::parse(message),
        "RemoveChild" => RemoveChildPayload::parse(message),
        "ListChildren" => ListChildrenPayload::parse(message),
        "SubscribeLayout" => SubscribeLayoutPayload::parse(message),
        "UnsubscribeLayout" => UnsubscribeLayoutPayload::parse(message),
        "SubscribeKeys" => SubscribeKeysPayload::parse(message),
        "UnsubscribeKeys" => UnsubscribeKeysPayload::parse(message),
        _ => Err(ParseError::UnknownType(type_field.into()))
    };
}

