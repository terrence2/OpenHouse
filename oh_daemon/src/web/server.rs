use actix_web::{http, server, App, HttpRequest, Path};
use web::resources::*;
use failure::Error;

pub fn run() -> Result<(), Error> {
    server::new(|| {
        App::new()
            .prefix("/gui")
            .route("/index.html", http::Method::GET, |_: HttpRequest| {
                index_html()
            })
    }).bind("127.0.0.1:8080")?
        .run();
    return Ok(());
}
