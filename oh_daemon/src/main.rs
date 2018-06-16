// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate actix;
extern crate actix_web;
#[macro_use]
extern crate approx;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate downcast_rs;
#[macro_use]
extern crate failure;
extern crate failure_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate openssl;
extern crate simplelog;
#[macro_use]
extern crate structopt;

mod tree;
mod web;

use actix::prelude::*;
use failure::Error;
use simplelog::{Config, LevelFilter, TermLogger};
use std::path::PathBuf;
use structopt::StructOpt;
use tree::Tree;
use web::server::build_server;

#[derive(StructOpt, Debug)]
#[structopt(name = "open_house")]
struct Opt {
    #[structopt(short = "d", long = "debug")]
    debug: bool,

    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,

    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    run(opt).unwrap();
}

fn run(opt: Opt) -> Result<(), Error> {
    let level = match opt.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    TermLogger::init(level, Config::default())?;

    let sys = System::new("open_house");

    let tree = Tree::new_empty().build_from_file(&opt.config)?;
    let _tree_addr: Addr<Unsync, _> = tree.start();

    let server = build_server("openhouse.eyrie", "127.0.0.1", 8089)?;
    let _server_addr: Addr<Syn, _> = server.start();

    //tree_addr.send(AddHandler())

    sys.run();
    return Ok(());
    //
    //    server::new(
    //        || App::new()
    //            .route("/gui/index.html", http::Method::GET, index))
    //        .bind("127.0.0.1:8080").unwrap()
    //        .run();
}
