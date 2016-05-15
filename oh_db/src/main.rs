// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate argparse;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate ws;

#[macro_use] mod utility;
mod message;
mod tree;

use std::rc::Rc;
use std::cell::RefCell;
use std::error::Error;
use rustc_serialize::json;
use tree::Tree;


fn main() {
    let mut log_level = "DEBUG".to_string();
    let mut log_target = "events.log".to_string();
    let mut address = "0.0.0.0".to_string();
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
        ap.parse_args_or_exit();
    }

    env_logger::init().unwrap();

    run_server(&address, port);
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
    ( $expr:expr, $conn:expr ) => {
        match $expr {
            Ok(a) => a,
            Err(e) => {
                return $conn.sender.borrow_mut().send(
                    format!(r#"{{ "status": "{}", "context": "{}" }}"#,
                            e.description(), e));
            }
        };
    };
}

fn run_server(address: &str, port: u16)
{
    struct Environment {
        // The database.
        db: Tree,
        // List of current connections.
        connections: Vec<Connection>
    }
    impl Environment {
        fn new() -> Self {
            Environment {
                db: Tree::new(),
                connections: Vec::new()
            }
        }
    }

    struct Connection {
        sender: Rc<RefCell<ws::Sender>>,
        env: Rc<RefCell<Environment>>
    }

    // Note that this clones the references: we obviously cannot clone
    // the connection itself or the global data structures we're sharing.
    impl Clone for Connection {
        fn clone(&self) -> Self {
            Connection {
                sender: self.sender.clone(),
                env: self.env.clone()
            }
        }
    }

    impl Connection {
        fn return_ok(&mut self) -> ws::Result<()> {
            self.sender.borrow_mut().send(r#"{ "status": "Ok" }"#)
        }

        fn handle_ping(&mut self, msg: &message::PingPayload) -> ws::Result<()> {
            info!("handling Ping -> {}", msg.data);
            let out = message::PingResponse { pong: msg.data.to_string() };
            let encoded = try_fatal!(json::encode(&out), self);
            return self.sender.borrow_mut().send(encoded.to_string());
        }

        fn handle_create_child(&mut self, msg: &message::CreateChildPayload) -> ws::Result<()> {
            info!("handling CreateChild -> parent: {},  name: {}", msg.parent_path, msg.name);
            {
                let db = &mut self.env.borrow_mut().db;
                let parent = try_error!(db.lookup(msg.parent_path.as_str()), self);
                try_error!(parent.add_child(msg.name.clone()), self);
            }
            self.return_ok()
        }

        fn handle_subscribe_key(&mut self, msg: &message::SubscribeKeyPayload) -> ws::Result<()> {
            info!("handling SubscribeKey -> {}[{}]", msg.path, msg.key);
            return Ok(());
        }
    }

    impl ws::Handler for Connection {
        fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
            let message_text = try!(msg.into_text());
            let data = try_fatal!(json::Json::from_str(&message_text), self);
            let message = try_fatal!(message::parse(data), self);
            match message {
                message::Message::Ping(ref payload) => {
                    self.handle_ping(payload)
                },
                message::Message::CreateChild(ref payload) => {
                    self.handle_create_child(payload)
                },
                message::Message::SubscribeKey(ref payload) => {
                    self.handle_subscribe_key(payload)
                },
                //_ => { self.sender.borrow_mut().shutdown() }
            }
        }

        fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
            info!("socket closing for ({:?}) {}", code, reason);
        }
    }

    let env: Rc<RefCell<Environment>> = Rc::new(RefCell::new(Environment::new()));

    // Start the server.
    if let Err(error) = ws::listen((address, port), move |sock| {
        let conn = Connection {
            sender: Rc::new(RefCell::new(sock)),
            env: env.clone()
        };
        env.borrow_mut().connections.push(conn.clone());
        return conn;
    }) {
        // Inform the user of failure
        error!("Failed to create WebSocket due to {:?}", error);
    }

    info!("SERVER: listen ended");
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
            run_server("127.0.0.1", 3013);
        });

        let mut connected = false;
        while !connected {
            match ws::connect("ws://127.0.0.1:3013", |ws| {
                    connected = true;
                    ws.send(r#"{"type": "Ping", "data": "hello"}"#).unwrap();
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
                if let Err(_) = ws::connect("ws://127.0.0.1:3013", move |ws| {
                    ws.send(r#"{"type": "Ping", "data": "hello"}"#).unwrap();
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
                if let Err(_) = ws::connect("ws://127.0.0.1:3013", move |ws| {
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

        clients = Vec::new();
        for parent in vec!["a", "b", "c", "d", "e", "f", "g", "h"] {
            let client = std::thread::spawn(move || {
                if let Err(_) = ws::connect("ws://127.0.0.1:3013", move |ws| {
                    for name in vec!["a", "b", "c", "d", "e", "f", "g", "h"] {
                        let msg = format!(r#"{{"type": "CreateChild",
                                               "parent_path": "{}",
                                               "name": "{}"}}"#,
                                          format!("/{}", parent), name);
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
}
