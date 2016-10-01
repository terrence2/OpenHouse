// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate argparse;
extern crate capnp;
extern crate env_logger;
extern crate ketos;
#[macro_use] extern crate log;
extern crate openssl;
extern crate rand;
extern crate ws;

#[macro_use] mod utility;
mod path;
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
use std::path::Path as FilePath;
use openssl::x509::X509FileType;
use openssl::ssl::{Ssl, SslContext, SslMethod,
                   SSL_VERIFY_PEER, SSL_VERIFY_FAIL_IF_NO_PEER_CERT};
use tree::{Tree, TreeChanges};
use path::PathBuilder;
use subscriptions::Watches;


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
               FilePath::new(&ca_chain),
               FilePath::new(&certificate),
               FilePath::new(&private_key)).unwrap();
}

fn run_server(address: &str, port: u16,
              ca_chain: &FilePath,
              certificate: &FilePath,
              private_key: &FilePath)
    -> ws::Result<()>
{
    let env = Rc::new(RefCell::new(Environment::new(ca_chain, certificate,
                                                    private_key)));

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

// Try; close the connection on failure. This should be reserved for client
// mistakes so severe that we cannot return an error: e.g. if we're not sure if
// we're even speaking the same protocol.
macro_rules! close_on_failure {
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

struct Environment<'e> {
    // The database.
    db: Tree,

    // Maps paths and keys to connections and subscription ids.
    watches: Watches,

    // Used to hand out unique subscription identifiers.
    last_subscription_id: u64,

    // List of current connections.
    connections: HashMap<ws::util::Token, Connection<'e>>,

    // The SSL configuration to use when establishing new connections.
    ssl_context: SslContext
}

impl<'e> Environment<'e> {
    fn new(ca_chain: &'e FilePath,
           certificate: &'e FilePath,
           private_key: &'e FilePath) -> Self
    {
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
            watches: Watches::new(),
            last_subscription_id: 0,
            connections: HashMap::new(),
            ssl_context: context
        }
    }

    // The connection triggering the event does not care about failures to send to
    // subscriptions, so this method terminates any failure. We log and potentially
    // close the child connections, but do not report failures to the caller.
    fn notify_subscriptions_glob(&mut self, changes: &TreeChanges)
    {
        let matching = self.watches.filter_changes_for_each_watch(changes);
        for (matching_changes, matching_conn, matching_sid) in  matching {
            let mut conn = self.connections.get_mut(&matching_conn).unwrap();
            conn.on_change(&matching_sid, &matching_changes).ok();
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
    fn handle_ping(&mut self, msg: &ping_request::Reader, response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let data = try!(msg.get_data());
        info!("handling Ping -> {}", data);
        let mut pong = response.init_ping();
        pong.set_pong(data);
        return Ok(());
    }

    fn handle_create_file(&mut self, msg: &create_file_request::Reader,
                          response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let parent_path = try!(try!(PathBuilder::new(try!(msg.get_parent_path())))
                               .finish_path());
        let name = try!(msg.get_name());
        info!("handling CreateFile -> parent: {},  name: {}", parent_path, name);
        let mut env = self.env.borrow_mut();
        let parent = try!(env.db.lookup_directory(&parent_path));
        try!(parent.add_file(&name));
        response.init_ok();
        return Ok(());
    }

    fn handle_create_formula(&mut self, msg: &create_formula_request::Reader,
                             response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let parent = try!(try!(PathBuilder::new(try!(msg.get_parent_path()))).finish_path());
        let name = try!(msg.get_name());
        let formula = try!(msg.get_formula());
        let mut inputs = HashMap::new();
        for input in try!(msg.get_inputs()).iter() {
            let input_path = try!(try!(PathBuilder::new(try!(input.get_path())))
                                  .finish_path());
            inputs.insert(try!(input.get_name()).to_owned(), input_path);
        }
        info!("handling CreateFormula: parent: {}, name: {}, inputs: {:?}, formula: {}",
              parent, name, inputs, formula);
        try!(self.env.borrow_mut().db.create_formula(&parent, &name, &inputs, &formula));
        response.init_ok();
        return Ok(());
    }

    fn handle_create_directory(&mut self, msg: &create_directory_request::Reader,
                               response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let parent_path = try!(try!(PathBuilder::new(try!(msg.get_parent_path())))
                               .finish_path());
        let name = try!(msg.get_name());
        info!("handling Createdirectory -> parent: {}, name: {}", parent_path, name);
        let mut env = self.env.borrow_mut();
        let parent = try!(env.db.lookup_directory(&parent_path));
        try!(parent.add_directory(&name));
        response.init_ok();
        return Ok(());
    }

    fn handle_remove_node(&mut self, msg: &remove_node_request::Reader,
                          response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let parent_path = try!(try!(PathBuilder::new(try!(msg.get_parent_path())))
                               .finish_path());
        let name = try!(msg.get_name());
        info!("handling RemoveNode-> parent: {}, name: {}", parent_path, name);
        let mut env = self.env.borrow_mut();
        let parent = try!(env.db.lookup_directory(&parent_path));
        try!(parent.remove_child(&name));
        response.init_ok();
        return Ok(());
    }

    fn handle_list_directory(&mut self, msg: &list_directory_request::Reader,
                             response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let path = try!(try!(PathBuilder::new(try!(msg.get_path()))).finish_path());
        info!("handling ListDirectory -> path: {}", path);
        let db = &mut self.env.borrow_mut().db;
        let directory = try!(db.lookup_directory(&path));
        let children = directory.list_directory();

        // Build the response.
        let ls_response = response.init_list_directory();
        let mut ls_children = ls_response.init_children(children.len() as u32);
        for (i, child) in children.iter().enumerate() {
            ls_children.set(i as u32, child)
        }
        return Ok(());
    }

    fn handle_get_file(&mut self, msg: &get_file_request::Reader,
                       response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let path = try!(try!(PathBuilder::new(try!(msg.get_path()))).finish_path());
        info!("handling GetFile -> path: {}", path);
        let db = &mut self.env.borrow_mut().db;
        let data = try!(db.get_data_at(&path));

        let mut cat_response = response.init_get_file();
        cat_response.set_data(&data);
        return Ok(());
    }

    fn handle_get_matching_files(&mut self, msg: &get_matching_files_request::Reader,
                                 response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let glob = try!(try!(PathBuilder::new(try!(msg.get_glob()))).finish_glob());
        info!("handling GetMatchingFiles -> glob: {}", glob);
        let db = &mut self.env.borrow_mut().db;
        let matches = try!(db.get_data_matching(&glob));

        let cat_response = response.init_get_matching_files();
        let mut cat_data = cat_response.init_data(matches.len() as u32);
        for (i, &ref match_pair) in matches.iter().enumerate() {
            cat_data.borrow().get(i as u32).set_path(&match_pair.0.to_str());
            cat_data.borrow().get(i as u32).set_data(&match_pair.1);
        }
        return Ok(());
    }

    fn handle_set_file(&mut self, msg: &set_file_request::Reader,
                       response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let path = try!(try!(PathBuilder::new(try!(msg.get_path()))).finish_path());
        let data = try!(msg.get_data());
        info!("handling SetFile -> path: {}, data: {}", path, data);
        let db = &mut self.env.borrow_mut().db;
        let changes = try!(db.set_data_at(&path, &data));
        self.env.borrow_mut().notify_subscriptions_glob(&changes);
        response.init_ok();
        return Ok(());
    }

    fn handle_set_matching_files(&mut self, msg: &set_matching_files_request::Reader,
                                 response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let glob = try!(try!(PathBuilder::new(try!(msg.get_glob()))).finish_glob());
        let data = try!(msg.get_data());
        info!("handling SetMatchingFiles -> glob: {}, data: {}", glob, data);
        let env = &mut self.env.borrow_mut();
        let changes = try!(env.db.set_data_matching(&glob, data));
        env.notify_subscriptions_glob(&changes);
        response.init_ok();
        return Ok(());
    }

    fn handle_watch_matching_files(&mut self, msg: &watch_matching_files_request::Reader,
                                   response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let glob = try!(try!(PathBuilder::new(try!(msg.get_glob()))).finish_glob());
        info!("handling WatchMatchingFiles -> glob: {}", glob);
        let mut env = self.env.borrow_mut();
        env.last_subscription_id += 1;
        let sid = SubscriptionId::from_u64(env.last_subscription_id);
        env.watches.add_watch(&sid, &self.sender.borrow().token(), &glob);
        let mut sub_response = response.init_watch();
        sub_response.set_subscription_id(sid.to_u64());
        return Ok(());
    }

    fn handle_unwatch(&mut self, msg: &unwatch_request::Reader,
                      response: server_response::Builder)
        -> Result<(), Box<Error>>
    {
        let sid = SubscriptionId::from_u64(msg.get_subscription_id());
        try!(self.env.borrow_mut().watches.remove_watch(&sid));
        response.init_ok();
        return Ok(());
    }

    fn on_change(&mut self, sid: &SubscriptionId, changes: &TreeChanges)
        -> ws::Result<()>
    {
        let mut builder = ::capnp::message::Builder::new_default();
        {
            let message = builder.init_root::<server_message::Builder>();
            let mut event = message.init_event();
            event.set_subscription_id(sid.to_u64());
            let mut changes_list = event.init_changes(changes.len() as u32);
            for (change_no, (data, paths)) in changes.iter().enumerate() {
                let mut change_ref = changes_list.borrow().get(change_no as u32);
                change_ref.set_data(data);
                let mut path_list = change_ref.init_paths(paths.len() as u32);
                for (i, path) in paths.iter().enumerate() {
                    path_list.set(i as u32, &path.to_str());
                }
            }
        }
        let mut buf = Vec::new();
        try!(capnp::serialize::write_message(&mut buf, &builder));
        return self.sender.borrow_mut().send(buf.as_slice());
    }
}

macro_rules! handle_client_request {
    (
        $kind:expr, $id:ident, $conn:expr, [ $( ($a:ident | $b:ident) ),* ]
    ) =>
    {
        match $kind {
            $(
                Ok(client_request::$a(req)) => {
                    let unwrapped = close_on_failure!(req, $conn);

                    let result;
                    let mut builder = ::capnp::message::Builder::new_default();
                    {
                        let message = builder.init_root::<server_message::Builder>();
                        let mut response = message.init_response();
                        response.set_id($id.to_u64());

                        // Note that since capnp's generated response objects'
                        // |self| only takes a copy, we *have* to move when
                        // calling our handler. This means that we need to
                        // process the result later when message is not pinning
                        // builder. We have to re-create the message, but not
                        // all the other machinery.
                        result = $conn.$b(&unwrapped, response);
                    }

                    // If we got an error, rebuild message as an error.
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            let message = builder.init_root::<server_message::Builder>();
                            let mut response = message.init_response();
                            response.set_id($id.to_u64());
                            let mut error_response = response.init_error();
                            error_response.set_name(e.description());
                            error_response.set_context(&format!("{}", e));
                        }
                    };

                    let mut buf = Vec::new();
                    try!(capnp::serialize::write_message(&mut buf, &builder));
                    return $conn.sender.borrow_mut().send(buf.as_slice());
                }
            ),*
            Err(e) => {
                close_on_failure!(Err(e), $conn);
            }
        }
    };
}

impl<'e> ws::Handler for Connection<'e> {
    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        if !msg.is_binary() {
            return self.sender.borrow_mut().close_with_reason(ws::CloseCode::Error,
                                                              "did not expect TEXT messages");
        }

        let message_data = msg.into_data();
        let message_reader = close_on_failure!(
            capnp::serialize::read_message(&mut std::io::Cursor::new(message_data),
                                           ::capnp::message::ReaderOptions::new()), self);
        let message = close_on_failure!(
            message_reader.get_root::<client_request::Reader>(), self);
        let message_id = MessageId::from_u64(message.get_id());
        handle_client_request!(message.which(), message_id, self,
                               [(Ping              | handle_ping),
                                (CreateFile        | handle_create_file),
                                (CreateFormula     | handle_create_formula),
                                (CreateDirectory   | handle_create_directory),
                                (RemoveNode        | handle_remove_node),
                                (GetFile           | handle_get_file),
                                (GetMatchingFiles  | handle_get_matching_files),
                                (SetFile           | handle_set_file),
                                (SetMatchingFiles  | handle_set_matching_files),
                                (ListDirectory     | handle_list_directory),
                                (WatchMatchingFiles| handle_watch_matching_files),
                                (Unwatch           | handle_unwatch)
                               ]);
        return Ok(());
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        info!("socket closing for ({:?}) {}", code, reason);
        self.env.borrow_mut().watches.remove_connection(&self.sender.borrow().token());
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
