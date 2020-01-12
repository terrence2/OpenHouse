// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{DBServer, HandleEvent};
use actix::prelude::*;
use actix_server::Server;
use actix_web::{
    dev, middleware, web, App, Error, FromRequest, HttpRequest, HttpResponse, HttpServer,
};
use bytes::Bytes;
use failure::{err_msg, Fallible};
use futures::future::{err, ok, Ready};
use log::trace;
//use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::{collections::HashMap, net::IpAddr, str};
use yggdrasil::Value;

struct AppState {
    db: Addr<DBServer>,
    button_path_map: HashMap<IpAddr, String>,
}

struct RequestIp {
    ip: IpAddr,
}

impl FromRequest for RequestIp {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;
    type Config = ();

    fn from_request(req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let conn_info = req.connection_info();
        let remote = conn_info.remote();
        if remote.is_none() {
            return err(err_msg("no remote host").into());
        }
        let addr = remote.unwrap().split(':').collect::<Vec<&str>>();
        if addr.first().is_none() {
            return err(err_msg("no address in remote host").into());
        }
        let ip = addr.first().unwrap().parse::<IpAddr>();
        if ip.is_err() {
            return err(err_msg("failed to parse remote host id").into());
        }
        let ip = ip.unwrap();
        trace!("server: mapped event to {}", ip);
        ok(RequestIp { ip })
    }
}

async fn handle_event(
    body: Bytes,
    app_data: web::Data<AppState>,
    request_ip: RequestIp,
) -> HttpResponse {
    let path = app_data.button_path_map.get(&request_ip.ip);
    if path.is_none() {
        return HttpResponse::NotFound().into();
    }
    let path = path.unwrap().to_string();
    let value = str::from_utf8(&body).unwrap().to_string();
    trace!("http server: recvd legacy mcu event {} <- {}", path, value);
    let event = HandleEvent {
        path,
        value: Value::String(value),
    };
    app_data.db.do_send(event);
    HttpResponse::Ok().finish()
}

pub fn build_server(
    db: Addr<DBServer>,
    button_path_map: HashMap<IpAddr, String>,
    hostname: &str,
    addr: &str,
    port: u16,
) -> Fallible<Server> {
    // let mut ssl_builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
    // ssl_builder.set_private_key_file("key.pem", SslFiletype::PEM)?;
    // ssl_builder.set_certificate_chain_file("cert.pem")?;

    let http_server = HttpServer::new(move || {
        App::new()
            .data(AppState {
                db: db.clone(),
                button_path_map: button_path_map.clone(),
            })
            .wrap(middleware::Logger::default())
            .service(web::resource("/event").route(web::post().to(handle_event)))
    })
    .server_hostname(hostname.to_string())
    .bind(&format!("{}:{}", addr, port))?;
    //.bind_ssl(&format!("{}:{}", addr, port), ssl_builder)?;
    let server = http_server.run();
    Ok(server)
}
