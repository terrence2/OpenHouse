// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
mod oh;

use failure::Fallible;
use oh::{ClockServer, HueServer, LegacyMcu, TreeServer};
use std::{net::IpAddr, path::PathBuf};
use structopt::StructOpt;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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

#[tokio::main(core_threads = 4)]
async fn main() -> Fallible<()> {
    let opt = Opt::from_args();
    let config = opt.config;
    let host = opt
        .host
        .unwrap_or_else(|| "127.0.0.1".to_string())
        .parse::<IpAddr>()?;
    let port = opt.port.unwrap_or(8090);

    let level = match opt.verbose {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?; //.expect("setting defualt subscriber failed");

    let tree_server = TreeServer::launch(&config).await?;
    let hue_server = HueServer::launch(tree_server.mailbox()).await?;
    let clock_server = ClockServer::launch(tree_server.mailbox()).await?;
    let legacy_mcu =
        LegacyMcu::launch(host, port, hue_server.mailbox(), tree_server.mailbox()).await?;

    signal::ctrl_c().await?;
    info!("ctrl-c received, shutting down cleanly");

    tree_server.mailbox().finish().await?;
    clock_server.mailbox().finish().await?;
    legacy_mcu.mailbox().finish().await?;
    hue_server.mailbox().finish().await?;

    clock_server.join().await?;
    legacy_mcu.join().await?;
    hue_server.join().await?;
    tree_server.join().await?;

    Ok(())
}
