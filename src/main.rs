use askama::Template;
use axum::{
    extract::Path,
    routing::{get, get_service, post},
    Router,
};
use landing::*;
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
mod purchase;
mod seating;

#[derive(Template)]
#[template(path = "temp.html")]
pub struct Temp {}

#[derive(Template)]
#[template(path = "booking.html")]
pub struct BookingPage {
    pub id: String,
}

const ADDR: &str = "127.0.0.1:8080";
const DB_ADDR: &str = "127.0.0.1:8000";
const ROOT: Root = Root {
    username: "root",
    password: "root",
};

static DB: Lazy<Surreal<Client>> = Lazy::new(Surreal::init);

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
        .route("/booking/:id", get(booking))
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

async fn movie(Path(_): Path<String>) -> Temp {
    Temp {}
}

async fn booking(Path(id): Path<String>) -> BookingPage {
    BookingPage { id }
}
