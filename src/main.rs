use askama::Template;
use axum::{
    routing::{get, get_service, post},
    Router,
};
use landing::*;
use lettre::{transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor};
use movie::*;
use once_cell::sync::Lazy;
use purchase::*;
use seating::*;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    Surreal,
};
use tower_http::services::ServeDir;

mod landing;
mod movie;
mod purchase;
mod seating;

#[derive(Template)]
#[template(path = "temp.html")]
pub struct Temp {}

const ADDR: &str = "127.0.0.1:8080";
const DB_ADDR: &str = "127.0.0.1:8000";
const ROOT: Root = Root {
    username: "root",
    password: "root",
};

static DB: Lazy<Surreal<Client>> = Lazy::new(Surreal::init);
static MAILER: Lazy<AsyncSmtpTransport<Tokio1Executor>> = Lazy::new(|| {
    AsyncSmtpTransport::<Tokio1Executor>::relay("email-smtp.us-east-2.amazonaws.com")
        .unwrap()
        .credentials(Credentials::new(
            "AKIAWUGJ4PUAFBE4PYTC".to_owned(),
            "BIO6CFyLoum70E6RxQCEOH+lW8t+iONyaROeGl5lVh4H".to_owned(),
        ))
        .build()
});

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    DB.connect::<Ws>(DB_ADDR).await?;
    DB.signin(ROOT).await?;
    DB.use_ns("theater").use_db("theater").await?;
    let purchase_routes = Router::new()
        .route("/:id/:seat", get(purchase))
        .route("/:id/:seat/:movie/:time", post(complete_purchase));

    let seating_routes = Router::new()
        .route("/:id", get(seating))
        .route("/:id/:seat", get(select_seat));

    let app = Router::new()
        .route("/", get(index))
        .route("/home", get(home))
        .route("/about", get(about))
        .route("/contact", get(contact))
        .route("/showtimes", get(showtimes))
        .route("/movie/:id", get(movie))
        .nest("/seating", seating_routes)
        .nest("/purchase", purchase_routes)
        .nest_service("/images", get_service(ServeDir::new("images")));

    println!("Listening on http://{ADDR}");

    axum::Server::bind(&ADDR.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use lettre::{
        message::header::ContentType, transport::smtp::authentication::Credentials, Message,
        SmtpTransport, Transport,
    };

    #[tokio::test]
    async fn email_test() {
        let creds = Credentials::new(
            "AKIAWUGJ4PUAFBE4PYTC".to_owned(),
            "BIO6CFyLoum70E6RxQCEOH+lW8t+iONyaROeGl5lVh4H".to_owned(),
        );
        let mailer = SmtpTransport::relay("email-smtp.us-east-2.amazonaws.com")
            .unwrap()
            .credentials(creds)
            .build();
        let email = Message::builder()
            .from("movietheatercsci694@yahoo.com".parse().unwrap())
            .to("theatertest@sharklasers.com".parse().unwrap())
            .subject("Hello World!")
            .header(ContentType::TEXT_PLAIN)
            .body(String::from("Hello World!"))
            .unwrap();
        let _ = mailer.send(&email).unwrap();
    }
}
