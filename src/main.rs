use account::*;
use askama::Template;
use axum::{
    extract::FromRef,
    routing::{get, get_service, post},
    Router,
};
use axum_extra::extract::cookie::Key;
use landing::*;
use login::*;
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

mod account;
mod landing;
mod login;
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

#[derive(Clone)]
struct AppState {
    key: Key,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    DB.connect::<Ws>(DB_ADDR).await?;
    DB.signin(ROOT).await?;
    DB.use_ns("theater").use_db("theater").await?;

    let state = AppState {
        key: Key::generate(),
    };

    let purchase_routes = Router::new()
        .route("/:id/:seat", get(purchase))
        .route("/:id/:seat/:movie/:time", post(complete_purchase));

    let seating_routes = Router::new()
        .route("/:id", get(seating))
        .route("/:id/:seat", get(select_seat))
        .route("/times/:id", get(times));

    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(get_login))
        .route("/login", post(post_login))
        .route("/logout", post(logout))
        .route("/sign_up", get(sign_up))
        .route("/sign_up", post(create_account))
        .route("/account", get(tickets))
        .route("/home", get(home))
        .route("/footer", get(footer))
        .route("/showtimes", get(showtimes))
        .route("/movie/:id", get(movie))
        .nest("/seating", seating_routes)
        .nest("/purchase", purchase_routes)
        .nest_service("/images", get_service(ServeDir::new("images")))
        .with_state(state);

    println!("Listening on http://{ADDR}");

    axum::Server::bind(&ADDR.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
