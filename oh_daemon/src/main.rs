// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate actix;
extern crate actix_web;
#[macro_use]
extern crate approx;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate simplelog;
#[macro_use]
extern crate structopt;

mod tree;
mod web;

use actix::prelude::*;
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;
use simplelog::{Config, LevelFilter, TermLogger};
use tree::TreeParser;

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
    TermLogger::init(LevelFilter::Warn, Config::default())?;

    let sys = System::new("open_house");

    let tree = TreeParser::from_file(&opt.config, opt.verbose)?;
    let tree_addr: Addr<Unsync, _> = tree.start();

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
