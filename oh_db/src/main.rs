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
mod proto_server;

pub mod messages_capnp {
    include!(concat!(env!("OUT_DIR"), "/messages_capnp.rs"));
}

use proto_server::ProtoServer;
use std::fmt;
use std::sync::Arc;
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

    let db_server = Arc::new(TreeServer::start_link().chain_err(|| "start tree server")?);
    let proto_handle = ProtoServer::run_server(port, db_server).chain_err(|| "start proto server")?;
    match proto_handle.join() {
        Ok(_) => (),
        Err(e) => bail!("join proto server failure: {}", format!("{:?}", e))
    };
    return Ok(());
}
