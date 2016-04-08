// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate argparse;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate ws;

use std::rc::Rc;
use std::cell::RefCell;
use argparse::{ArgumentParser, Store};
use rustc_serialize::json;
use ws::{listen, CloseCode, Error, Handler, Message, Result, Sender};

mod message;


fn main() {
    let mut log_level = "DEBUG".to_string();
    let mut log_target = "events.log".to_string();
    let mut address = "0.0.0.0".to_string();
    let mut port = 8182;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("The OpenHouse central database.");
        ap.refer(&mut log_level)
          .add_option(&["-l", "--log-level"], Store,
                      "The logging level. (default DEBUG)");
        ap.refer(&mut log_target)
          .add_option(&["-L", "--log-target"], Store,
                      "The logging target. (default events.log)");
        ap.refer(&mut address)
          .add_option(&["-a", "--address"], Store,
                      "The address to listen on. (default 0.0.0.0)");
        ap.refer(&mut port)
          .add_option(&["-p", "--port"], Store,
                      "The port to listen on. (default 8887)");
        ap.parse_args_or_exit();
    }

    env_logger::init().unwrap();

    run_server(&address, port);
}

macro_rules! try_json {
    ( $expr : expr ) => {
        match $expr {
            Ok(a) => a,
            Err(e) => return Err(Error::from(Box::new(e)))
        };
    };
}

fn run_server(address: &str, port: u16) {
    struct Connection {
        ws: Sender,
    }

    // We cannot store the Rc<RefCell<>> directly in heap storage because we
    // need to implement ws::Handler on it and we are only allowed to implement
    // traits for locally defined structures. Thus, we have to wrap it.
    // Fortunately, this does give us a nice place to implement Handler.
    struct HeapConnection {
        conn: Rc<RefCell<Connection>>
    }

    impl Connection {
        fn handle_ping(&mut self, msg: &message::PingPayload) -> Result<()> {
            info!("handling PING: {}", msg.data);
            let out = message::PingResponse { pong: msg.data.to_string() };
            let encoded = try_json!(json::encode(&out));
            let rv = self.ws.send(encoded.to_string());
            return rv;
        }
        /*
        fn handle_unknown(&mut self, type_field: &str) {
            //self.ws.send("{\"error\": \"unknown 'type' field: " + type_field + "\"}");
        }
        */
    }

    impl Handler for HeapConnection {
        fn on_message(&mut self, msg: Message) -> Result<()> {
            //info!("{:?} RECV '{}'.", self.address, msg);
            let message_text = try!(msg.into_text());
            let data = try_json!(json::Json::from_str(&message_text));
            let message = try_json!(message::parse(data));
            match message {
                message::Message::Ping(ref payload) => {
                    self.conn.borrow_mut().handle_ping(payload)
                },
                //SubscribeMessage(sub) => do_sub(sub)
                _ => { self.conn.borrow_mut().ws.shutdown() }
            }
        }

        fn on_close(&mut self, code: CloseCode, reason: &str) {
            info!("socket closing for ({:?}) {}", code, reason);
        }
    }

    // Listen on an address and call the closure for each connection
    let mut connections: Vec<Rc<RefCell<Connection>>> = Vec::new();
    if let Err(error) = listen((address, port), |ws| {
        let conn = Connection { ws: ws };
        let boxed = Rc::new(RefCell::new(conn));
        connections.push(boxed.clone());
        return HeapConnection { conn: boxed };
    }) {
        // Inform the user of failure
        println!("Failed to create WebSocket due to {:?}", error);
    }

    println!("SERVER: listen ended");
}


#[cfg(test)]
mod tests {
    extern crate env_logger;
    extern crate rustc_serialize;
    extern crate std;
    extern crate ws;

    use super::run_server;
    use rustc_serialize::json;
    use ws::Message;

    /*
    #[test]
    fn it_connects() {
        let _ = env_logger::init();

        let server = std::thread::spawn(move || {
            run_server("127.0.0.1", 3012);
        });

        std::thread::sleep(std::time::Duration::from_millis(100));

        let client = std::thread::spawn(move || {
            if let Err(_) = ws::connect("ws://127.0.0.1:3012", |ws| {
                ws.send("hello").unwrap();
                move |_| {
                    ws.close(ws::CloseCode::Normal)
                }
            }) {
                assert!(false);
            }
        });

        let _ = server.join();
        let _ = client.join();
    }
    */

    #[test]
    fn it_pings() {
        let _ = env_logger::init();

        std::thread::spawn(move || {
            run_server("127.0.0.1", 3013);
        });

        let mut connected = false;
        while !connected {
            match ws::connect("ws://127.0.0.1:3013", |ws| {
                    connected = true;
                    ws.send("{\"type\": \"ping\", \"ping\": \"hello\"}").unwrap();
                    return move |_| {
                        return ws.close(ws::CloseCode::Normal);
                    };
                }) {
                Ok(_) => {}
                Err(_) => {}
            }
        }

        let mut clients = Vec::new();
        for _ in 0..10 {
            let client = std::thread::spawn(move || {
                if let Err(_) = ws::connect("ws://127.0.0.1:3013", |ws| {
                    ws.send("{\"type\": \"ping\", \"ping\": \"hello\"}").unwrap();
                    return move |msg: Message| {
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
}
