// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::prelude::*;
use actix_web::{
    http::Method, middleware, server, server::Server, App, FutureResponse, HttpMessage,
    HttpRequest, HttpResponse,
};
use bytes::Bytes;
use failure::{err_msg, Error, Fallible};
use futures::future::{ok, Future};
use oh::{DBServer, HandleEvent};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::{collections::HashMap, net::IpAddr, str};

struct AppState {
    db: Addr<DBServer>,
    button_path_map: HashMap<IpAddr, String>,
}

fn get_caller_ip(req: &HttpRequest<AppState>) -> Fallible<IpAddr> {
    let info = req.connection_info();
    let remote_host = info.remote();
    let ip = remote_host
        .ok_or_else(|| err_msg("cannot find remote host for event"))?
        .split(":")
        .collect::<Vec<&str>>()
        .first()
        .ok_or_else(|| err_msg("remote host is empty in event"))?
        .parse::<IpAddr>()?;
    return Ok(ip);
}

fn handle_event(req: &HttpRequest<AppState>) -> FutureResponse<HttpResponse> {
    let ip = match get_caller_ip(req) {
        Ok(ip) => ip,
        Err(e) => return Box::new(ok(HttpResponse::BadRequest().finish())),
    };

    let path = req.state().button_path_map.get(&ip);
    if path.is_none() {
        return Box::new(ok(HttpResponse::NotFound().finish()));
    }

    let path = path.unwrap().to_string();
    let db = req.state().db.clone();
    return Box::new(
        req.body()
            .limit(128)
            .from_err()
            .and_then(move |bytes: Bytes| {
                println!("==== BODY ==== {:?}", bytes);
                let value = str::from_utf8(&bytes).unwrap().to_string();
                db.send(HandleEvent { path, value });
                ok(HttpResponse::Ok().into())
            }),
    );
}

pub fn build_server(
    db: Addr<DBServer>,
    button_path_map: HashMap<IpAddr, String>,
    hostname: &str,
    addr: &str,
    port: u16,
) -> Fallible<Addr<Server>> {
    let mut ssl_builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
    ssl_builder.set_private_key_file("key.pem", SslFiletype::PEM)?;
    ssl_builder.set_certificate_chain_file("cert.pem")?;

    // return Ok(server::new(move || {
    //     build_app(AppState {
    //         db,
    //         button_path_map,
    //     })
    // }).server_hostname(addr.to_owned())
    //     .bind_ssl(&format!("{}:{}", addr, port), ssl_builder)?
    //     .start());

    let server = server::new(move || {
        App::with_state(AppState {
            db: db.clone(),
            button_path_map: button_path_map.clone(),
        }).middleware(middleware::Logger::default())
            // .route("/index.html", http::Method::GET, |_: HttpRequest| {
            //     index_html()
            // })
            // .resource("/", |r| {
            //     r.method(http::Method::GET).f(|req| {
            //         HttpResponse::Found()
            //             .header("LOCATION", "/index.html")
            //             .finish()
            //     })
            // })

            // let app = App::new().resource("/index.html", |r| {
            //     r.method(http::Method::GET)
            //            .with_config(index, |cfg| { // <- register handler with extractor params
            //               cfg.0.limit(4096);  // <- limit size of the payload
            //             })
            // });

            .resource("/event", |res| {
                res.method(Method::POST).a(|req: &HttpRequest<AppState>| -> FutureResponse<HttpResponse> {
                    return handle_event(req);
                })
            })
    }).server_hostname(hostname.to_string())
        .bind_ssl(&format!("{}:{}", addr, port), ssl_builder)?;
    return Ok(server.start());
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_build() -> Fallible<()> {
        return Ok(());
    }
}
