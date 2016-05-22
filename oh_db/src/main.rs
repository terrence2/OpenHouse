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
                    format!(r#"{{ "id": "{}", "status": "{}", "context": "{}" }}"#,
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
                connections: HashMap::new(),
                ssl_context: context
            }
        }
    }

    struct Connection<'e> {
        sender: Rc<RefCell<ws::Sender>>,
        env: Rc<RefCell<Environment<'e>>>
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
        fn return_ok(&mut self, id: u64) -> ws::Result<()> {
            self.sender.borrow_mut().send(
                format!(r#"{{ "id": "{}", "status": "Ok" }}"#, id))
        }

        fn handle_ping(&mut self, id: u64, msg: &PingPayload) -> ws::Result<()> {
            info!("handling Ping -> {}", msg.data);
            let out = PingResponse { id: id, pong: msg.data.to_string() };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_create_child(&mut self, id: u64, msg: &CreateChildPayload)
            -> ws::Result<()>
        {
            info!("handling CreateChild -> parent: {},  name: {}",
                  msg.parent_path, msg.name);
            {
                let db = &mut self.env.borrow_mut().db;
                let parent = try_error!(db.lookup(msg.parent_path.as_str()), id, self);
                try_error!(parent.add_child(msg.name.clone()), id, self);
            }
            self.return_ok(id)
        }

        fn handle_remove_child(&mut self, id: u64, msg: &RemoveChildPayload)
            -> ws::Result<()>
        {
            info!("handling RemoveChild -> parent: {},  name: {}",
                  msg.parent_path, msg.name);
            {
                let db = &mut self.env.borrow_mut().db;
                let parent = try_error!(db.lookup(msg.parent_path.as_str()), id, self);
                try_error!(parent.remove_child(msg.name.clone()), id, self);
            }
            self.return_ok(id)
        }

        fn handle_list_children(&mut self, id: u64, msg: &ListChildrenPayload)
            -> ws::Result<()>
        {
            info!("handling ListChildren -> path: {}", msg.path);
            let db = &mut self.env.borrow_mut().db;
            let node = try_error!(db.lookup(msg.path.as_str()), id, self);
            let children = node.list_children();
            let out = ListChildrenResponse {
                id: id,
                status: String::from("Ok"),
                children: children
            };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_subscribe_key(&mut self, id: u64, msg: &SubscribeKeyPayload) -> ws::Result<()> {
            info!("handling SubscribeKey -> {}[{}]", msg.path, msg.key);
            return Ok(());
        }
    }

    impl<'e> ws::Handler for Connection<'e> {
        fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
            let message_text = try!(msg.into_text());
            let data = try_error!(json::Json::from_str(&message_text), 0, self);
            let message = try_error!(parse_message(data), 0, self);
            match message {
                Message::Ping(id, ref payload) => {
                    self.handle_ping(id, payload)
                },
                Message::CreateChild(id, ref payload) => {
                    self.handle_create_child(id, payload)
                },
                Message::RemoveChild(id, ref payload) => {
                    self.handle_remove_child(id, payload)
                },
                Message::ListChildren(id, ref payload) => {
                    self.handle_list_children(id, payload)
                },
                Message::SubscribeKey(id, ref payload) => {
                    self.handle_subscribe_key(id, payload)
                },
                //_ => { self.sender.borrow_mut().shutdown() }
            }
        }

        fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
            info!("socket closing for ({:?}) {}", code, reason);
            self.env.borrow_mut().connections.remove(&self.sender.borrow().token());
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


#[cfg(test)]
mod tests {
    extern crate env_logger;
    extern crate rustc_serialize;
    extern crate std;
    extern crate ws;

    use super::run_server;
    use rustc_serialize::json;

    fn launch_server_thread() {
        std::thread::spawn(move || {
            run_server("127.0.0.1", 3013).unwrap();
        });

        let mut connected = false;
        while !connected {
            match ws::connect("wss://127.0.0.1:3013", |ws| {
                    connected = true;
                    ws.send(r#"{"id": 1, "type": "Ping", "data": "hello"}"#).unwrap();
                    return move |_| {
                        return ws.close(ws::CloseCode::Normal);
                    };
                }) {
                Ok(_) => {}
                Err(_) => {}
            }
        }
    }

    #[test]
    fn it_pings() {
        let _ = env_logger::init();

        launch_server_thread();

        let mut clients = Vec::new();
        for _ in 0..10 {
            let client = std::thread::spawn(move || {
                if let Err(_) = ws::connect("wss://127.0.0.1:3013", move |ws| {
                    ws.send(r#"{"id": 1, "type": "Ping", "data": "hello"}"#).unwrap();
                    return move |msg: ws::Message| {
                        assert!(msg.is_text());
                        let data = json::Json::from_str(&msg.into_text().unwrap()).unwrap();
                        let pong = data.find("pong").unwrap();
                        assert!(pong == &json::Json::String("hello".to_string()));
                        return ws.close(ws::CloseCode::Normal);
                    };
                }) {
                    assert!(false);
                }
            });
            clients.push(client);
        }

        for client in clients {
            let result = client.join();
            assert!(!result.is_err());
        }
    }

    /*
    fn expect_status_ok(msg: ws::Message) {
        assert!(msg.is_text());
        let data = json::Json::from_str(&msg.into_text().unwrap()).unwrap();
        let status = data.find("status").unwrap();
        assert!(status == &json::Json::String("Ok".to_string()));
    }

    #[test]
    fn it_creates_nodes() {
        let _ = env_logger::init();

        launch_server_thread();

        struct Client {
            sender: ws::Sender
        }
        impl ws::Handler for Client {
            fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
                expect_status_ok(msg);
                return self.sender.close(ws::CloseCode::Normal);
            }
            fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
                assert!(code == ws::CloseCode::Normal, String::from(reason));
            }
        }
        let mut clients = Vec::new();
        for name in vec!["a", "b", "c", "d", "e", "f", "g", "h"] {
            let client = std::thread::spawn(move || {
                if let Err(_) = ws::connect("wss://127.0.0.1:3013", move |ws| {
                    let msg = format!(r#"{{"type": "CreateChild",
                                           "parent_path": "{}",
                                           "name": "{}"}}"#,
                                      "/", name);
                    ws.send(msg).unwrap();
                    return Client { sender: ws };
                }) {
                    assert!(false);
                }
            });
            clients.push(client);
        }

        for client in clients {
            let result = client.join();
            assert!(!result.is_err());
        }

        let mut id: u64 = 1;
        clients = Vec::new();
        for parent in vec!["a", "b", "c", "d", "e", "f", "g", "h"] {
            let client = std::thread::spawn(move || {
                if let Err(_) = ws::connect("wss://127.0.0.1:3013", move |ws| {
                    for name in vec!["a", "b", "c", "d", "e", "f", "g", "h"] {
                        let msg = format!(r#"{{"id": {},
                                               "type": "CreateChild",
                                               "parent_path": "{}",
                                               "name": "{}"}}"#,
                                          id, format!("/{}", parent), name);
                        id += 1;
                        ws.send(msg).unwrap();
                    }
                    return Client { sender: ws };
                }) {
                    assert!(false);
                }
            });
            clients.push(client);
        }

        for client in clients {
            let result = client.join();
            assert!(!result.is_err());
        }
    }
    */
}
