// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
mod oh;
mod web;

use actix::prelude::*;
use failure::Fallible;
use oh::{DBServer, LegacyMCU, TickWorker};
use std::path::PathBuf;
use structopt::StructOpt;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use web::server::build_server;

#[derive(StructOpt, Debug)]
#[structopt(name = "open_house")]
struct Opt {
    #[structopt(short = "d", long = "debug")]
    debug: bool,

    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,

    #[structopt(short = "h", long = "host")]
    host: Option<String>,

    #[structopt(short = "p", long = "port")]
    port: Option<u16>,

    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config: PathBuf,
}

fn main() -> Fallible<()> {
    let opt = Opt::from_args();

    let level = match opt.verbose {
        0 => Level::Info,
        1 => Level::Debug,
        _ => Level::Trace,
    };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting defualt subscriber failed");

    let sys = System::new("open_house");

    let db = DBServer::new_from_file(&opt.config)?;
    let button_path_map = db
        .legacy_mcu
        .inspect_as(&|mcu: &LegacyMCU| &mcu.path_map)?
        .clone();
    let db_addr = db.start();

    let ticker = TickWorker::new(&db_addr);
    let _tick_addr = ticker.start();

    let _server_server = build_server(
        db_addr,
        button_path_map,
        "openhouse.eyrie",
        &opt.host.unwrap_or_else(|| "localhost".to_string()),
        opt.port.unwrap_or(8090),
    )?;
    //let _server_addr = server.start();
    //tree_addr.send(AddHandler())

    sys.run()?;
    Ok(())
}
