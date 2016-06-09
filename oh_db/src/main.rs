// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate argparse;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate openssl;
extern crate rand;
extern crate rustc_serialize;
extern crate ws;

#[macro_use] mod utility;
mod message;
mod subscriptions;
mod tree;

use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use openssl::x509::X509FileType;
use openssl::ssl::{Ssl, SslContext, SslMethod,
                   SSL_VERIFY_PEER, SSL_VERIFY_FAIL_IF_NO_PEER_CERT};
use rustc_serialize::json;
use tree::Tree;
use message::*;
use subscriptions::Subscriptions;


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
                return $conn.sender.borrow_mut().send(
                    format!(r#"{{ "message_id": "{}", "status": "{}", "context": "{}" }}"#,
                            $id, e.description(), e));
            }
        };
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
        fn notify_layout_subscriptions(&mut self, path: &Path, event: &str, name: &str) {
            for (token, layout_sid) in self.subscriptions.get_layout_subscriptions_for(path) {
                // If this connection does not exist, then something is way off the rails
                // and we need to shutdown anyway.
                let mut conn = self.connections.get_mut(&token).unwrap();
                conn.on_layout_changed(&layout_sid, path, event, name).ok();
            }
        }

        fn notify_keys_subscriptions(&mut self, path: &Path, event: &str, name: &str) {
            for (token, keys_sid) in self.subscriptions.get_keys_subscriptions_for(path) {
                // If this connection does not exist, then something is way off the rails
                // and we need to shutdown anyway.
                let mut conn = self.connections.get_mut(&token).unwrap();
                conn.on_keys_changed(&keys_sid, path, event, name).ok();
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
            self.sender.borrow_mut().send(
                format!(r#"{{ "message_id": "{}", "status": "Ok" }}"#, message_id))
        }

        fn handle_ping(&mut self, message_id: MessageId, msg: &PingPayload) -> ws::Result<()> {
            info!("handling Ping -> {}", msg.data);
            let out = PingResponse { message_id: message_id, pong: msg.data.to_string() };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_create_child(&mut self, message_id: MessageId, msg: &CreateChildPayload)
            -> ws::Result<()>
        {
            info!("handling CreateChild -> parent: {},  name: {}",
                  msg.parent_path, msg.name);
            {
                let mut env = self.env.borrow_mut();
                let parent_path = Path::new(msg.parent_path.as_str());
                {
                    let db = &mut env.db;
                    let parent = try_error!(db.lookup(parent_path), message_id, self);
                    try_error!(parent.add_child(&msg.name), message_id, self);
                }
                env.notify_layout_subscriptions(parent_path, "Create", msg.name.as_str());
            }
            self.return_ok(message_id)
        }

        fn handle_remove_child(&mut self, message_id: MessageId, msg: &RemoveChildPayload)
            -> ws::Result<()>
        {
            info!("handling RemoveChild -> parent: {},  name: {}",
                  msg.parent_path, msg.name);
            {
                let mut env = self.env.borrow_mut();
                let parent_path = Path::new(msg.parent_path.as_str());
                {
                    // Before removing, check that we won't be orphaning subscriptions.
                    try_error!(tree::check_path_component(&msg.name), message_id, self);
                    let mut path = parent_path.to_owned();
                    path.push(&msg.name);
                    try_error!(env.subscriptions.verify_no_subscriptions_at_path(&path), message_id, self);
                }
                {
                    let db = &mut env.db;
                    let parent = try_error!(db.lookup(parent_path), message_id, self);
                    try_error!(parent.remove_child(&msg.name), message_id, self);
                }
                env.notify_layout_subscriptions(parent_path, "Remove", msg.name.as_str());
            }
            self.return_ok(message_id)
        }

        fn handle_list_children(&mut self, message_id: MessageId, msg: &ListChildrenPayload)
            -> ws::Result<()>
        {
            info!("handling ListChildren -> path: {}", msg.path);
            let db = &mut self.env.borrow_mut().db;
            let path = Path::new(msg.path.as_str());
            let node = try_error!(db.lookup(path), message_id, self);
            let children = node.list_children();
            let out = ListChildrenResponse {
                message_id: message_id,
                status: String::from("Ok"),
                children: children
            };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_subscribe_layout(&mut self, message_id: MessageId, msg: &SubscribeLayoutPayload)
            -> ws::Result<()>
        {
            info!("handling SubscribeLayout -> path: {}", msg.path);
            let path = Path::new(msg.path.as_str());
            let mut env = self.env.borrow_mut();
            {
                // Look up the node to ensure that it exists.
                let _ = try_error!(env.db.lookup(path), message_id, self);
            }
            env.last_subscription_id += 1;
            let sid = LayoutSubscriptionId::from_u64(env.last_subscription_id);
            env.subscriptions.add_layout_subscription(&sid, &self.sender.borrow().token(), &path);
            let out = SubscribeLayoutResponse {
                message_id: message_id,
                status: String::from("Ok"),
                layout_subscription_id: sid
            };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_unsubscribe_layout(&mut self, message_id: MessageId,
                                     msg: &UnsubscribeLayoutPayload)
            -> ws::Result<()>
        {
            {
                let mut env = self.env.borrow_mut();
                let sid = &msg.layout_subscription_id;
                try_error!(env.subscriptions.remove_layout_subscription(sid), message_id, self);
            }
            self.return_ok(message_id)
        }

        fn on_layout_changed(&mut self, layout_sid: &LayoutSubscriptionId, path: &Path,
                             event: &str, name: &str) -> ws::Result<()>
        {
            let message = SubscribeLayoutMessage {
                layout_subscription_id: *layout_sid,
                path: path.to_string_lossy().into_owned(),
                event: event.to_owned(),
                name: name.to_owned()
            };
            let encoded = try_fatal!(json::encode(&message), self);
            return self.sender.borrow_mut().send(encoded);
        }

        fn handle_create_key(&mut self, message_id: MessageId, msg: &CreateChildPayload)
            -> ws::Result<()>
        {
            info!("handling CreateKey -> path: {}, key: {}, value: {}",
                  msg.parent_path, msg.key, msg.value);
            {
                let mut env = self.env.borrow_mut();
                let parent_path = Path::new(msg.parent_path.as_str());
                {
                    let db = &mut env.db;
                    let parent = try_error!(db.lookup(parent_path), message_id, self);
                    try_error!(parent.create_key(&msg.name), message_id, self);
                }
                env.notify_layout_subscriptions(parent_path, "Create", msg.name.as_str());
            }
            self.return_ok(message_id)
        }

        /*
        fn handle_remove_key(&mut self, message_id: MessageId, msg: &RemoveChildPayload)
            -> ws::Result<()>
        {
            info!("handling RemoveChild -> parent: {},  name: {}",
                  msg.parent_path, msg.name);
            {
                let mut env = self.env.borrow_mut();
                let parent_path = Path::new(msg.parent_path.as_str());
                {
                    // Before removing, check that we won't be orphaning subscriptions.
                    try_error!(tree::check_path_component(&msg.name), message_id, self);
                    let mut path = parent_path.to_owned();
                    path.push(&msg.name);
                    try_error!(env.subscriptions.verify_no_subscriptions_at_path(&path), message_id, self);
                }
                {
                    let db = &mut env.db;
                    let parent = try_error!(db.lookup(parent_path), message_id, self);
                    try_error!(parent.remove_key(&msg.name), message_id, self);
                }
                env.notify_layout_subscriptions(parent_path, "Remove", msg.name.as_str());
            }
            self.return_ok(message_id)
        }
        */

        fn handle_subscribe_keys(&mut self, message_id: MessageId, msg: &SubscribeKeysPayload)
            -> ws::Result<()>
        {
            info!("handling SubscribeKeys -> path: {}", msg.path);
            let path = Path::new(msg.path.as_str());
            let mut env = self.env.borrow_mut();
            {
                // Look up the node to ensure that it exists.
                let _ = try_error!(env.db.lookup(path), message_id, self);
            }
            env.last_subscription_id += 1;
            let sid = KeysSubscriptionId::from_u64(env.last_subscription_id);
            env.subscriptions.add_keys_subscription(&sid, &self.sender.borrow().token(), &path);
            let out = SubscribeKeysResponse {
                message_id: message_id,
                status: String::from("Ok"),
                keys_subscription_id: sid
            };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_unsubscribe_keys(&mut self, message_id: MessageId,
                                   msg: &UnsubscribeKeysPayload)
            -> ws::Result<()>
        {
            {
                let mut env = self.env.borrow_mut();
                let sid = &msg.keys_subscription_id;
                try_error!(env.subscriptions.remove_keys_subscription(sid), message_id, self);
            }
            self.return_ok(message_id)
        }

        fn on_keys_changed(&mut self, keys_sid: &KeysSubscriptionId, path: &Path,
                           event: &str, name: &str) -> ws::Result<()>
        {
            let message = SubscribeKeysMessage {
                keys_subscription_id: *keys_sid,
                path: path.to_string_lossy().into_owned(),
                event: event.to_owned(),
                name: name.to_owned()
            };
            let encoded = try_fatal!(json::encode(&message), self);
            return self.sender.borrow_mut().send(encoded);
        }
    }

    impl<'e> ws::Handler for Connection<'e> {
        fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
            let message_text = try!(msg.into_text());
            let data = try_fatal!(json::Json::from_str(&message_text), self);
            let message_id = try_fatal!(parse_message_id(&data), self);
            let message = try_error!(parse_message(&data), message_id, self);
            match message {
                Message::Ping(ref payload) => {
                    self.handle_ping(message_id, payload)
                },
                Message::CreateChild(ref payload) => {
                    self.handle_create_child(message_id, payload)
                },
                Message::RemoveChild(ref payload) => {
                    self.handle_remove_child(message_id, payload)
                },
                Message::ListChildren(ref payload) => {
                    self.handle_list_children(message_id, payload)
                },
                Message::SubscribeLayout(ref payload) => {
                    self.handle_subscribe_layout(message_id, payload)
                },
                Message::UnsubscribeLayout(ref payload) => {
                    self.handle_unsubscribe_layout(message_id, payload)
                },
                Message::SubscribeKeys(ref payload) => {
                    self.handle_subscribe_keys(message_id, payload)
                },
                Message::UnsubscribeKeys(ref payload) => {
                    self.handle_unsubscribe_keys(message_id, payload)
                },
                //_ => { self.sender.borrow_mut().shutdown() }
            }
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

    try!(template.listen((address, port)));
    info!("SERVER: listen ended");
    return Ok(());
}
