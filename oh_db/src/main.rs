// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate argparse;
extern crate capnp;
extern crate env_logger;
#[macro_use] extern crate log;
extern crate openssl;
extern crate rand;
extern crate ws;

#[macro_use] mod utility;
//mod message;
mod subscriptions;
mod tree;

pub mod messages_capnp {
    include!(concat!(env!("OUT_DIR"), "/messages_capnp.rs"));
}

use messages_capnp::*;

use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use openssl::x509::X509FileType;
use openssl::ssl::{Ssl, SslContext, SslMethod,
                   SSL_VERIFY_PEER, SSL_VERIFY_FAIL_IF_NO_PEER_CERT};
use tree::Tree;
//use message::*;
use subscriptions::Subscriptions;


make_identifier!(MessageId);
make_identifier!(SubscriptionId);


fn main() {
    let mut log_level = "DEBUG".to_string();
    let mut log_target = "events.log".to_string();
    let mut address = "0.0.0.0".to_string();
    let mut ca_chain = "".to_string();
    let mut certificate = "".to_string();
    let mut private_key = "".to_string();
    let mut port = 8182;
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("The OpenHouse central database.");
        ap.refer(&mut log_level)
          .add_option(&["-l", "--log-level"], argparse::Store,
                      "The logging level. (default DEBUG)");
        ap.refer(&mut log_target)
          .add_option(&["-L", "--log-target"], argparse::Store,
                      "The logging target. (default events.log)");
        ap.refer(&mut address)
          .add_option(&["-a", "--address"], argparse::Store,
                      "The address to listen on. (default 0.0.0.0)");
        ap.refer(&mut port)
          .add_option(&["-p", "--port"], argparse::Store,
                      "The port to listen on. (default 8887)");
        ap.refer(&mut ca_chain)
          .add_option(&["-C", "--ca-chain"], argparse::Store,
                      "The authority chain to use. (required)");
        ap.refer(&mut certificate)
          .add_option(&["-c", "--certificate"], argparse::Store,
                      "The public key for connections. (required)");
        ap.refer(&mut private_key)
          .add_option(&["-k", "--private-key"], argparse::Store,
                      "The private key for connections. (required)");
        ap.parse_args_or_exit();
    }

    if ca_chain == "" {
        panic!(concat!("A certificate authority trust chain must be specified to verify ",
                       "client connections. Please pass -C or --ca-chain with the trust ",
                       "chain used to sign clients we expect to accept."));
    }
    if certificate == "" {
        panic!(concat!("A certificate (public key) must be specified for use with SSL. ",
                       "Please use -c or --certificiate to provide a PEM encoded file to ",
                       "use as the certificate to present to client connections."));
    }
    if private_key == "" {
        panic!(concat!("A private key matching the given certificate must be provided ",
                       "with -k or --private-key so that we can communicate with clients."));
    }

    env_logger::init().unwrap();

    info!("oh_db Version {}", env!("CARGO_PKG_VERSION"));
    info!("Using {}", openssl::version::version());

    run_server(&address, port,
               Path::new(&ca_chain),
               Path::new(&certificate),
               Path::new(&private_key)).unwrap();
}

// Try and close the connection on failure. This should be reserved for client
// mistakes such as invalid formats and such.
macro_rules! try_fatal {
    ( $expr : expr, $conn : expr ) => {
        match $expr {
            Ok(a) => a,
            Err(e) => {
                return $conn.sender.borrow_mut().close_with_reason(
                    ws::CloseCode::Error,
                    format!("{}", e));
            }
        };
    };
}

// Try and send an error to the client on failure. This should be used for
// any recoverable error.
macro_rules! try_error {
    ( $expr:expr, $id:expr, $conn:expr ) => {
        match $expr {
            Ok(a) => a,
            Err(e) => {
                // FIXME: simplify this somehow.
                let mut builder = ::capnp::message::Builder::new_default();
                {
                    let message = builder.init_root::<server_message::Builder>();
                    let mut response = message.init_response();
                    response.set_id($id.to_u64());
                    // // //
                    let mut error_response = response.init_error();
                    error_response.set_name(e.description());
                    error_response.set_context(&format!("{}", e));
                    // // //
                }
                let mut buf = Vec::new();
                try!(capnp::serialize_packed::write_message(&mut buf, &builder));
                return $conn.sender.borrow_mut().send(buf.as_slice());
            }
        };
    };
}

impl fmt::Display for create_node_request::NodeType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            create_node_request::NodeType::File => write!(f, "NodeType::File"),
            create_node_request::NodeType::Directory => write!(f, "NodeType::Directory")
        }
    }
}

macro_rules! handle_client_request {
    (
        $kind:expr, $id:ident, $conn:ident;
        [ $( ($a:ident | $b:ident) ),* ]
    ) =>
    {
        match $kind {
            $(
                Ok(client_request::$a(req)) => {
                    let unwrapped = try_fatal!(req, $conn);
                    return $conn.$b($id, &unwrapped);
                }
            ),*
            Err(e) => {
                try_error!(Err(e), $id, $conn);
            }
        }
    };
}

fn run_server(address: &str, port: u16, ca_chain: &Path, certificate: &Path, private_key: &Path)
    -> ws::Result<()>
{
    struct Environment<'e> {
        // The database.
        db: Tree,

        // Maps paths and keys to connections and subscription ids.
        subscriptions: Subscriptions,

        // Used to hand out unique subscription identifiers.
        last_subscription_id: u64,

        // List of current connections.
        connections: HashMap<ws::util::Token, Connection<'e>>,

        // The SSL configuration to use when establishing new connections.
        ssl_context: SslContext
    }

    impl<'e> Environment<'e> {
        fn new(ca_chain: &'e Path, certificate: &'e Path, private_key: &'e Path) -> Self {
            let mut context = SslContext::new(SslMethod::Tlsv1_2).unwrap();

            // Verify peer certificates.
            context.set_verify(SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, None);
            context.set_verify_depth(std::u32::MAX);

            // Enable our way to more security.
            context.set_options(openssl::ssl::SSL_OP_SINGLE_DH_USE |
                                openssl::ssl::SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION |
                                openssl::ssl::SSL_OP_NO_TICKET);

            // Set a session id because that's required.
            let mut session_ctx: [u8;32] = [0;32];
            for i in 0..32 { session_ctx[i] = rand::random::<u8>(); }
            context.set_session_id_context(&session_ctx).unwrap();  // must be set for client certs.

            // Set our certificate paths.
            context.set_CA_file(ca_chain).unwrap();  // set trust authority to our CA
            context.set_certificate_file(certificate, X509FileType::PEM).unwrap();
            context.set_private_key_file(private_key, X509FileType::PEM).unwrap();
            context.check_private_key().unwrap();  // check consistency of cert and key

            // Use EC if possible.
            context.set_ecdh_auto(true).unwrap();  // needed for forward security.

            // Only support the one cipher we want to use.
            context.set_cipher_list("ECDHE-RSA-AES256-GCM-SHA384").unwrap();

            Environment {
                db: Tree::new(),
                subscriptions: Subscriptions::new(),
                last_subscription_id: 0,
                connections: HashMap::new(),
                ssl_context: context
            }
        }

        // The connection triggering the event does not care about failures to send to
        // subscriptions, so this method terminates any failure. We log and potentially
        // close the child connections, but do not report failures to the caller.
        fn notify_subscriptions(&mut self, path: &Path, kind: EventKind, context: &str)
        {
            for (token, sid) in self.subscriptions.get_subscriptions_for(path) {
                // If this connection does not exist, then something is way off the rails
                // and we need to shutdown anyway.
                let mut conn = self.connections.get_mut(&token).unwrap();
                conn.on_change(&sid, path, kind, context).ok();
            }
        }
    }

    struct Connection<'e> {
        // A reference to our shared environment.
        //
        // Note that each mio context runs in its own thread. This means that our server instance
        // is single threaded, so that it is always safe to take a borrow_mut() from these. We only
        // need the Rc<RefCell>> because rust cannot see through mio's OS calls.
        env: Rc<RefCell<Environment<'e>>>,

        // The websocket itself.
        sender: Rc<RefCell<ws::Sender>>,
    }

    // Note that this clones the references: we obviously cannot clone
    // the connection itself or the global data structures we're sharing.
    impl<'e> Clone for Connection<'e> {
        fn clone(&self) -> Self {
            Connection {
                sender: self.sender.clone(),
                env: self.env.clone()
            }
        }
    }

    impl<'e> Connection<'e> {
        fn return_ok(&mut self, message_id: MessageId) -> ws::Result<()> {
            // FIXME: simplify this somehow.
            let mut builder = ::capnp::message::Builder::new_default();
            {
                let message = builder.init_root::<server_message::Builder>();
                let mut response = message.init_response();
                response.set_id(message_id.to_u64());
                // // //
                response.init_ok();
                // // //
            }
            let mut buf = Vec::new();
            try!(capnp::serialize_packed::write_message(&mut buf, &builder));
            return self.sender.borrow_mut().send(buf.as_slice());
        }

        fn handle_ping(&mut self, message_id: MessageId, msg: &ping_request::Reader)
            -> ws::Result<()>
        {
            let data = try_error!(msg.get_data(), message_id, self);
            info!("handling Ping -> {}", data);
            let mut buf = Vec::new();
            let mut builder = capnp::message::Builder::new_default();
            {
                let message = builder.init_root::<server_message::Builder>();
                let mut response = message.init_response();
                response.set_id(message_id.to_u64());
                // // //
                let mut pong = response.init_ping();
                pong.set_pong(data);
                // // //
            }
            try!(capnp::serialize_packed::write_message(&mut buf, &builder));
            return self.sender.borrow_mut().send(buf.as_slice());
        }

        fn handle_create_node(&mut self, message_id: MessageId, msg: &create_node_request::Reader)
            -> ws::Result<()>
        {
            let parent_path = Path::new(try_error!(msg.get_parent_path(), message_id, self));
            let name = try_error!(msg.get_name(), message_id, self);
            let node_type = try_error!(msg.get_node_type(), message_id, self);
            info!("handling CreateNode -> parent: {},  name: {}, type: {}",
                  parent_path.display(), name, node_type);
            {
                let mut env = self.env.borrow_mut();
                {
                    let db = &mut env.db;
                    let parent = try_error!(db.lookup_directory(parent_path), message_id, self);
                    try_error!(match node_type {
                        create_node_request::NodeType::Directory => parent.add_directory(&name),
                        create_node_request::NodeType::File => parent.add_file(&name)
                    }, message_id, self);
                }
                env.notify_subscriptions(parent_path, EventKind::Created, name);
            }
            self.return_ok(message_id)
        }

        fn handle_remove_node(&mut self, message_id: MessageId, msg: &remove_node_request::Reader)
            -> ws::Result<()>
        {
            let parent_path = try_error!(msg.get_parent_path(), message_id, self);
            let name = try_error!(msg.get_name(), message_id, self);
            info!("handling RemoveNode-> parent: {}, name: {}", parent_path, name);
            {
                let mut env = self.env.borrow_mut();
                let parent_path = Path::new(parent_path);
                {
                    // Before removing, check that we won't be orphaning subscriptions.
                    try_error!(tree::check_path_component(&name), message_id, self);
                    let mut path = parent_path.to_owned();
                    path.push(&name);
                    try_error!(env.subscriptions.verify_no_subscriptions_at_path(&path), message_id, self);
                }
                {
                    let db = &mut env.db;
                    let parent = try_error!(db.lookup_directory(parent_path), message_id, self);
                    try_error!(parent.remove_child(&name), message_id, self);
                }
                env.notify_subscriptions(parent_path, EventKind::Removed, name);
            }
            self.return_ok(message_id)
        }

        fn handle_list_directory(&mut self, message_id: MessageId, msg: &list_directory_request::Reader)
            -> ws::Result<()>
        {
            let path = Path::new(try_error!(msg.get_path(), message_id, self));
            info!("handling ListDirectory -> path: {}", path.display());
            let db = &mut self.env.borrow_mut().db;
            let directory = try_error!(db.lookup_directory(path), message_id, self);
            let children = directory.list_directory();

            // FIXME: simplify this somehow.
            let mut builder = ::capnp::message::Builder::new_default();
            {
                let message = builder.init_root::<server_message::Builder>();
                let mut response = message.init_response();
                response.set_id(message_id.to_u64());
                // // //
                let ls_response = response.init_list_directory();
                let mut ls_children = ls_response.init_children(children.len() as u32);
                for (i, child) in children.iter().enumerate() {
                    ls_children.set(i as u32, child)
                }
                // // //
            }
            let mut buf = Vec::new();
            try!(capnp::serialize_packed::write_message(&mut buf, &builder));
            return self.sender.borrow_mut().send(buf.as_slice());
        }

        fn handle_get_file_content(&mut self, message_id: MessageId, msg: &get_file_content_request::Reader)
            -> ws::Result<()>
        {
            let path = Path::new(try_error!(msg.get_path(), message_id, self));
            info!("handling GetFileContent -> path: {}", path.display());
            let data;
            {
                let db = &mut self.env.borrow_mut().db;
                let file = try_error!(db.lookup_file(path), message_id, self);
                data = file.get_data();
            }

            // FIXME: simplify this somehow.
            let mut builder = ::capnp::message::Builder::new_default();
            {
                let message = builder.init_root::<server_message::Builder>();
                let mut response = message.init_response();
                response.set_id(message_id.to_u64());
                // // //
                let mut cat_response = response.init_get_file_content();
                cat_response.set_data(&data);
                // // //
            }
            let mut buf = Vec::new();
            try!(capnp::serialize_packed::write_message(&mut buf, &builder));
            return self.sender.borrow_mut().send(buf.as_slice());
        }

        fn handle_set_file_content(&mut self, message_id: MessageId, msg: &set_file_content_request::Reader)
            -> ws::Result<()>
        {
            let path = try_error!(msg.get_path(), message_id, self);
            let data = try_error!(msg.get_data(), message_id, self);
            info!("handling SetFileContent -> path: {}", path);
            let path = Path::new(path);
            {
                let db = &mut self.env.borrow_mut().db;
                let file = try_error!(db.lookup_file(path), message_id, self);
                file.set_data(&data);
            }
            self.env.borrow_mut().notify_subscriptions(path, EventKind::Changed, &data);
            self.return_ok(message_id)
        }

        fn handle_subscribe(&mut self, message_id: MessageId, msg: &subscribe_request::Reader)
            -> ws::Result<()>
        {
            let path = Path::new(try_error!(msg.get_path(), message_id, self));
            info!("handling Subscribe -> path: {}", path.display());
            let mut env = self.env.borrow_mut();
            {
                // Look up the node to ensure that it exists.
                let _ = try_error!(env.db.contains_path(path), message_id, self);
            }
            env.last_subscription_id += 1;
            let sid = SubscriptionId::from_u64(env.last_subscription_id);
            env.subscriptions.add_subscription(&sid, &self.sender.borrow().token(), &path);

            // FIXME: simplify this somehow.
            let mut builder = ::capnp::message::Builder::new_default();
            {
                let message = builder.init_root::<server_message::Builder>();
                let mut response = message.init_response();
                response.set_id(message_id.to_u64());
                // // //
                let mut sub_response = response.init_subscribe();
                sub_response.set_subscription_id(sid.to_u64());
                // // //
            }
            let mut buf = Vec::new();
            try!(capnp::serialize_packed::write_message(&mut buf, &builder));
            return self.sender.borrow_mut().send(buf.as_slice());
        }

        fn handle_unsubscribe(&mut self, message_id: MessageId, msg: &unsubscribe_request::Reader)
            -> ws::Result<()>
        {
            let sid = SubscriptionId::from_u64(msg.get_subscription_id());
            {
                let mut env = self.env.borrow_mut();
                try_error!(env.subscriptions.remove_subscription(&sid), message_id, self);
            }
            self.return_ok(message_id)
        }

        fn on_change(&mut self, sid: &SubscriptionId, path: &Path, kind: EventKind, context: &str)
            -> ws::Result<()>
        {
            let mut builder = ::capnp::message::Builder::new_default();
            {
                let message = builder.init_root::<server_message::Builder>();
                let mut event = message.init_event();
                event.set_subscription_id(sid.to_u64());
                event.set_path(&path.to_string_lossy().into_owned());
                event.set_kind(kind);
                event.set_context(context);
            }
            let mut buf = Vec::new();
            try!(capnp::serialize_packed::write_message(&mut buf, &builder));
            return self.sender.borrow_mut().send(buf.as_slice());
        }
    }

    impl<'e> ws::Handler for Connection<'e> {
        fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
            if !msg.is_binary() {
                return self.sender.borrow_mut().close_with_reason(ws::CloseCode::Error,
                                                                  "did not expect TEXT messages");
            }

            let message_data = msg.into_data();
            let message_reader = try_fatal!(
                capnp::serialize_packed::read_message(&mut std::io::Cursor::new(message_data),
                                               ::capnp::message::ReaderOptions::new()), self);
            let message = try_fatal!(
                message_reader.get_root::<client_request::Reader>(), self);
            let message_id = MessageId::from_u64(message.get_id());
            handle_client_request!(message.which(), message_id, self;
                                   [(Ping | handle_ping),
                                    (CreateNode | handle_create_node),
                                    (RemoveNode | handle_remove_node),
                                    (GetFileContent | handle_get_file_content),
                                    (SetFileContent | handle_set_file_content),
                                    (ListDirectory | handle_list_directory),
                                    (Subscribe | handle_subscribe),
                                    (Unsubscribe | handle_unsubscribe)
                                   ]);

            /*
            match message.which() {

                Ok(client_request::Ping(req)) => {
                    let ping_req = try_error!(req, message_id, self);
                    return self.handle_ping(message_id, &ping_req)
                },
                Ok(client_request::CreateNode(req)) => {
                    let create_node_req = try_error!(req, message_id, self);
                    return self.handle_create_node(message_id, &create_node_req);
                },
                _ => { return Ok(()); }
            }
            */


            return Ok(());
            /*
            let message_text = try!(msg.into_text());
            let data = try_fatal!(json::Json::from_str(&message_text), self);
            let message_id = try_fatal!(parse_message_id(&data), self);
            let message = try_error!(parse_message(&data), message_id, self);
            match message {
                Message::Ping(ref payload) => {
                    self.handle_ping(message_id, payload)
                },
                Message::CreateNode(ref payload) => {
                    self.handle_create_node(message_id, payload)
                },
                Message::RemoveNode(ref payload) => {
                    self.handle_remove_node(message_id, payload)
                },
                Message::GetFileContent(ref payload) => {
                    self.handle_get_file_content(message_id, payload)
                },
                Message::SetFileContent(ref payload) => {
                    self.handle_set_file_content(message_id, payload)
                },
                Message::ListDirectory(ref payload) => {
                    self.handle_list_directory(message_id, payload)
                },
                Message::Subscribe(ref payload) => {
                    self.handle_subscribe(message_id, payload)
                },
                Message::Unsubscribe(ref payload) => {
                    self.handle_unsubscribe(message_id, payload)
                },
                //_ => { self.sender.borrow_mut().shutdown() }
            }
            */
        }

        fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
            info!("socket closing for ({:?}) {}", code, reason);
            self.env.borrow_mut().subscriptions.remove_connection(&self.sender.borrow().token());
        }

        fn build_ssl(&mut self) -> ws::Result<Ssl> {
            info!("building OpenSSL session for new connection");
            match Ssl::new(&self.env.borrow().ssl_context) {
                Ok(a) => return Ok(a),
                Err(e) => {
                    // Close the connection if SSL session creation fails.
                    self.sender.borrow_mut().close_with_reason(
                        ws::CloseCode::Error, format!("{}", e)).ok();
                    return Err(ws::Error::new(ws::ErrorKind::Ssl(e), "ssl session create failed"));
                }
            }
        }
    }

    let env: Rc<RefCell<Environment>> = Rc::new(RefCell::new(
            Environment::new(ca_chain, certificate, private_key)));

    // Start the server.
    let mut settings = ws::Settings::default();
    settings.method_strict = true;
    settings.masking_strict = true;
    settings.key_strict = true;
    settings.encrypt_server = true;

    let template = try!(ws::Builder::new().with_settings(settings).build(move |sock| {
        let conn = Connection {
            sender: Rc::new(RefCell::new(sock)),
            env: env.clone()
        };
        env.borrow_mut().connections.insert(conn.sender.borrow().token(), conn.clone());
        return conn;
    }));

    info!("Starting server on {}:{}", address, port);
    try!(template.listen((address, port)));
    info!("SERVER: listen ended");
    return Ok(());
}
