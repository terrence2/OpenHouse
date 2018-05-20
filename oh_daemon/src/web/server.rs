use actix_web::{http, middleware, server, App, HttpRequest};
use failure::Error;
use openssl::ssl::{SslAcceptor, SslMethod};
use web::resources::*;

pub fn build_server(
    hostname: &str,
    addr: &str,
    port: u16,
) -> Result<server::HttpServer<App>, Error> {
    let ssl_builder = SslAcceptor::mozilla_modern(SslMethod::dtls())?;
    return Ok(server::new(|| build_app())
        .server_hostname(hostname.to_owned())
        .bind_ssl(&format!("{}:{}", addr, port), ssl_builder)?);
}

fn build_app() -> Vec<App> {
    vec![
        App::new()
            .middleware(middleware::Logger::default())
            .prefix("/app")
            .route("/index.html", http::Method::GET, |_: HttpRequest| {
                index_html()
            }),
    ]
}
