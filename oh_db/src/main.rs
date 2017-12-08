// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate argparse;
extern crate capnp;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate ketos;
#[macro_use]
extern crate log;
extern crate otp;
extern crate rand;
extern crate ws;
extern crate yggdrasil;

#[macro_use]
mod utility;
mod subscriptions;
mod tree_server;

pub mod messages_capnp {
    include!(concat!(env!("OUT_DIR"), "/messages_capnp.rs"));
}

use messages_capnp::*;

use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use yggdrasil::{Glob, Path, TreeChanges};
use subscriptions::Watches;
use tree_server::TreeServer;

pub mod errors {
    error_chain!{
        foreign_links {
            Capnp(::capnp::Error);
        }
    }
}
use errors::{ResultExt, Result};

make_identifier!(MessageId);
make_identifier!(SubscriptionId);


quick_main!(run);
fn run() -> Result<()> {
    let mut log_level = "DEBUG".to_string();
    let mut log_target = "events.log".to_string();
    let mut port = 8182;
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("The OpenHouse central database.");
        ap.refer(&mut log_level).add_option(
            &["-l", "--log-level"],
            argparse::Store,
            "The logging level. (default DEBUG)",
        );
        ap.refer(&mut log_target).add_option(
            &["-L", "--log-target"],
            argparse::Store,
            "The logging target. (default events.log)",
        );
        ap.refer(&mut port).add_option(
            &["-b", "--bind"],
            argparse::Store,
            "The port to listen on. (default 8182)",
        );
        ap.parse_args_or_exit();
    }

    env_logger::init().unwrap();

    info!("oh_db Version {}", env!("CARGO_PKG_VERSION"));

    run_server(port).unwrap();
    return Ok(());
}

fn run_server(port: u16) -> Result<()> {
    let env = Rc::new(RefCell::new(
        Environment::new().chain_err(|| "new environment")?));

    // Start the server.
    let mut settings = ws::Settings::default();
    settings.method_strict = true;
    settings.masking_strict = true;
    settings.key_strict = true;

    let template = ws::Builder::new().with_settings(settings).build(
        move |sock| {
            let conn = Connection {
                sender: Rc::new(RefCell::new(sock)),
                env: env.clone(),
            };
            env.borrow_mut().connections.insert(
                conn.sender.borrow().token(),
                conn.clone(),
            );
            return conn;
        },
    ).chain_err(|| "build ws server")?;

    info!("Starting server on 127.0.0.1:{}", port);
    template.listen(("127.0.0.1", port)).chain_err(|| "ws listen")?;
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
    db_server: TreeServer,

    // Maps paths and keys to connections and subscription ids.
    watches: Watches,

    // Used to hand out unique subscription identifiers.
    last_subscription_id: u64,

    // List of current connections.
    connections: HashMap<ws::util::Token, Connection<'e>>,
}

impl<'e> Environment<'e> {
    fn new() -> Result<Self> {
        Ok(Environment {
            db_server: TreeServer::start_link().chain_err(|| "start tree server")?,
            watches: Watches::new(),
            last_subscription_id: 0,
            connections: HashMap::new(),
        })
    }

    // The connection triggering the event does not care about failures to send to
    // subscriptions, so this method terminates any failure. We log and potentially
    // close the child connections, but do not report failures to the caller.
    fn notify_subscriptions_glob(&mut self, changes: &TreeChanges) {
        let matching = self.watches.filter_changes_for_each_watch(changes);
        for (matching_changes, matching_conn, matching_sid) in matching {
            let conn = self.connections.get_mut(&matching_conn).unwrap();
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
            env: self.env.clone(),
        }
    }
}

impl<'e> Connection<'e> {
    fn handle_ping(
        &mut self,
        msg: &ping_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let data = msg.get_data()?;
        info!("handling Ping -> {}", data);
        let mut pong = response.init_ping();
        pong.set_pong(data);
        return Ok(());
    }

    fn handle_create_file(
        &mut self,
        msg: &create_file_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let parent_path_str = msg.get_parent_path().chain_err(|| "parent path str")?;
        let parent_path = Path::parse(parent_path_str).chain_err(|| "parent path")?;
        let name = msg.get_name()?;
        info!(
            "handling CreateFile -> parent: {},  name: {}",
            parent_path,
            name
        );
        self.env.borrow().db_server.create_file(parent_path, name)
            .chain_err(|| "create_file")?;
        response.init_ok();
        return Ok(());
    }

    fn handle_create_formula(
        &mut self,
        msg: &create_formula_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let parent_path_str = msg.get_parent_path().chain_err(|| "parent path str")?;
        let parent_path = Path::parse(parent_path_str).chain_err(|| "parent path")?;
        let name = msg.get_name()?;
        let formula = msg.get_formula()?;
        let mut inputs = HashMap::new();
        for input in msg.get_inputs()?.iter() {
            let input_path_str = input.get_path().chain_err(|| "input path str")?;
            let input_path = Path::parse(input_path_str).chain_err(|| "input path")?;
            inputs.insert(input.get_name()?.to_owned(), input_path);
        }
        info!(
            "handling CreateFormula: parent: {}, name: {}, inputs: {:?}, formula: {}",
            parent_path,
            name,
            inputs,
            formula
        );
        self.env.borrow().db_server
            .create_formula(parent_path, name, inputs, formula)
            .chain_err(|| "create_formula")?;
        response.init_ok();
        return Ok(());
    }

    fn handle_create_directory(
        &mut self,
        msg: &create_directory_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let parent_path_str = msg.get_parent_path().chain_err(|| "parent path str")?;
        let parent_path = Path::parse(parent_path_str).chain_err(|| "parent path")?;
        let name = msg.get_name()?;
        info!(
            "handling Createdirectory -> parent: {}, name: {}",
            parent_path,
            name
        );
        self.env.borrow().db_server.create_directory(parent_path, name).chain_err(|| "create directory")?;
        response.init_ok();
        return Ok(());
    }

    fn handle_remove_node(
        &mut self,
        msg: &remove_node_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let parent_path_str = msg.get_parent_path().chain_err(|| "parent path str")?;
        let parent_path = Path::parse(parent_path_str).chain_err(|| "parent path")?;
        let name = msg.get_name()?;
        info!(
            "handling RemoveNode-> parent: {}, name: {}",
            parent_path,
            name
        );
        self.env.borrow().db_server.remove_node(parent_path, name).chain_err(|| "remove node")?;
        response.init_ok();
        return Ok(());
    }

    fn handle_list_directory(
        &mut self,
        msg: &list_directory_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let path_str = msg.get_path().chain_err(|| "parent path str")?;
        let path = Path::parse(path_str).chain_err(|| "parent path")?;
        info!("handling ListDirectory -> path: {}", path);
        let children = self.env.borrow().db_server.list_directory(path).chain_err(|| "list directory")?;

        // Build the response.
        let ls_response = response.init_list_directory();
        let mut ls_children = ls_response.init_children(children.len() as u32);
        for (i, child) in children.iter().enumerate() {
            ls_children.set(i as u32, child)
        }
        return Ok(());
    }

    fn handle_get_file(
        &mut self,
        msg: &get_file_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let path_str = msg.get_path().chain_err(|| "parent path str")?;
        let path = Path::parse(path_str).chain_err(|| "parent path")?;
        info!("handling GetFile -> path: {}", path);
        let data = self.env.borrow().db_server.get_file(path).chain_err(|| "get file")?;
        let mut cat_response = response.init_get_file();
        cat_response.set_data(&data);
        return Ok(());
    }

    fn handle_get_matching_files(
        &mut self,
        msg: &get_matching_files_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let glob_str = msg.get_glob().chain_err(|| "glob str")?;
        let glob = Glob::parse(glob_str).chain_err(|| "glob")?;
        info!("handling GetMatchingFiles -> glob: {}", glob);
        let cat_response = response.init_get_matching_files();
        let matches = self.env.borrow().db_server.get_matching_files(glob).chain_err(|| "get matching files")?;
        let mut cat_data = cat_response.init_data(matches.len() as u32);
        for (i, &ref match_pair) in matches.iter().enumerate() {
            cat_data.borrow().get(i as u32).set_path(
                &match_pair.0.to_str(),
            );
            cat_data.borrow().get(i as u32).set_data(&match_pair.1);
        }
        return Ok(());
    }

    fn handle_set_file(
        &mut self,
        msg: &set_file_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let path_str = msg.get_path().chain_err(|| "parent path str")?;
        let path = Path::parse(path_str).chain_err(|| "parent path")?;
        let data = msg.get_data().chain_err(|| "get message data")?;
        info!("handling SetFile -> path: {}, data: {}", path, data);
        let changes = self.env.borrow().db_server.set_file(path, data).chain_err(|| "set file")?;
        self.env.borrow_mut().notify_subscriptions_glob(&changes);
        response.init_ok();
        return Ok(());
    }

    fn handle_set_matching_files(
        &mut self,
        msg: &set_matching_files_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let glob_str = msg.get_glob().chain_err(|| "glob str")?;
        let glob = Glob::parse(glob_str).chain_err(|| "glob")?;
        let data = msg.get_data()?;
        info!(
            "handling SetMatchingFiles -> glob: {}, data: {}",
            glob,
            data
        );
        let changes = self.env.borrow().db_server.set_matching_files(glob, data).chain_err(|| "set data matching")?;
        self.env.borrow_mut().notify_subscriptions_glob(&changes);
        response.init_ok();
        return Ok(());
    }

    fn handle_watch_matching_files(
        &mut self,
        msg: &watch_matching_files_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let glob_str = msg.get_glob().chain_err(|| "glob str")?;
        let glob = Glob::parse(glob_str).chain_err(|| "glob")?;
        info!("handling WatchMatchingFiles -> glob: {}", glob);
        let mut env = self.env.borrow_mut();
        env.last_subscription_id += 1;
        let sid = SubscriptionId::from_u64(env.last_subscription_id);
        env.watches.add_watch(
            &sid,
            &self.sender.borrow().token(),
            &glob,
        );
        let mut sub_response = response.init_watch();
        sub_response.set_subscription_id(sid.to_u64());
        return Ok(());
    }

    fn handle_unwatch(
        &mut self,
        msg: &unwatch_request::Reader,
        response: server_response::Builder,
    ) -> Result<()> {
        let sid = SubscriptionId::from_u64(msg.get_subscription_id());
        {
            let mut env = self.env.borrow_mut();
            env.watches.remove_watch(&sid).chain_err(|| "remove watch")?;
        }
        response.init_ok();
        return Ok(());
    }

    fn on_change(&mut self, sid: &SubscriptionId, changes: &TreeChanges) -> ws::Result<()> {
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
        capnp::serialize::write_message(&mut buf, &builder)?;
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

                        // Note that since capnp's generated response objects' |self| only
                        // takes a copy, we *have* to move when calling our handler. This means
                        // that we need to process the result later when message is not pinning
                        // builder. We have to re-create the message, but not all the other
                        // machinery.
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
                    capnp::serialize::write_message(&mut buf, &builder)?;
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
            return self.sender.borrow_mut().close_with_reason(
                ws::CloseCode::Error,
                "did not expect TEXT messages",
            );
        }

        let message_data = msg.into_data();
        let message_reader = close_on_failure!(
            capnp::serialize::read_message(
                &mut std::io::Cursor::new(message_data),
                ::capnp::message::ReaderOptions::new(),
            ),
            self
        );
        let message = close_on_failure!(message_reader.get_root::<client_request::Reader>(), self);
        let message_id = MessageId::from_u64(message.get_id());
        handle_client_request!(
            message.which(),
            message_id,
            self,
            [
                (Ping | handle_ping),
                (CreateFile | handle_create_file),
                (CreateFormula | handle_create_formula),
                (CreateDirectory | handle_create_directory),
                (RemoveNode | handle_remove_node),
                (GetFile | handle_get_file),
                (GetMatchingFiles | handle_get_matching_files),
                (SetFile | handle_set_file),
                (SetMatchingFiles | handle_set_matching_files),
                (ListDirectory | handle_list_directory),
                (WatchMatchingFiles | handle_watch_matching_files),
                (Unwatch | handle_unwatch)
            ]
        );
        return Ok(());
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        info!("socket closing for ({:?}) {}", code, reason);
        self.env.borrow_mut().watches.remove_connection(
            &self.sender.borrow().token(),
        );
    }
}
