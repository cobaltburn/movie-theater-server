use askama::Template;
use axum::{
    extract::Path,
    http::StatusCode,
    response::Result,
    routing::{get, get_service},
    Router,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    sql::Thing,
    Surreal,
};
use tower_http::services::ServeDir;

const ADDR: &str = "127.0.0.1:8080";
const DB_ADDR: &str = "127.0.0.1:8000";
const ROOT: Root = Root {
    username: "root",
    password: "root",
};

#[derive(Template)]
#[template(path = "temp.html")]
struct Temp {}

#[derive(Template)]
#[template(path = "index.html")]
struct Index {}

#[derive(Template)]
#[template(path = "home.html")]
struct Home {
    movies: Vec<Movie>,
}

#[derive(Template)]
#[template(path = "contact.html")]
struct Contact {}

#[derive(Template)]
#[template(path = "about.html")]
struct About {}

#[derive(Template)]
#[template(path = "showtime.html")]
struct Showtime {
    movies: Vec<MovieShowTimes>,
}

#[derive(Template)]
#[template(path = "booking.html")]
struct Booking {
    id: String,
}

#[derive(Template)]
#[template(path = "seating.html")]
struct Seating {
    id: String,
    seats: Vec<Seat>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Seat {
    available: bool,
    seat: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    id: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
struct Movie {
    id: Thing,
    name: Arc<str>,
    genres: Vec<Arc<str>>,
    runtime: i32,
    tagline: Arc<str>,
    stars: f32,
    description: Arc<str>,
    image: Arc<str>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MovieShowTimes {
    id: Thing,
    name: Arc<str>,
    genres: Vec<Arc<str>>,
    runtime: i32,
    tagline: Arc<str>,
    stars: f32,
    description: Arc<str>,
    image: Arc<str>,
    times: Vec<Time>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Time {
    id: Thing,
    time: Arc<str>,
}

static DB: Lazy<Surreal<Client>> = Lazy::new(Surreal::init);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    DB.connect::<Ws>(DB_ADDR).await?;
    DB.signin(ROOT).await?;
    DB.use_ns("theater").use_db("theater").await?;

    let app = Router::new()
        .route("/", get(index))
        .route("/home", get(home))
        .route("/showtimes", get(showtimes))
        .route("/about", get(about))
        .route("/contact", get(contact))
        .route("/movie/:id", get(movie))
        .route("/booking/:id", get(booking))
        .route("/seating/:id", get(seating))
        .route("/seating/:id/:seat", get(select_seat))
        .nest_service("/images", get_service(ServeDir::new("images")));

    println!("Listening on http://{ADDR}");

    axum::Server::bind(&ADDR.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

async fn index() -> Index {
    Index {}
}

async fn home() -> Result<Home> {
    if let Ok(movies) = DB.select("movies").await {
        return Ok(Home { movies });
    }
    Err(StatusCode::NOT_FOUND.into())
}

async fn showtimes() -> Result<Showtime> {
    let movies = DB
        .query(
            r#"
            SELECT *, (
                SELECT time::format(time, "%k:%M") AS time, id 
                FROM ->?->?->?->showtime 
                ORDER BY time
            ) AS times
            FROM movies
            "#,
        )
        .await;

    if let Ok(movies) = movies.expect("invalid query in showtimes").take(0) {
        return Ok(Showtime { movies });
    }
    Err(StatusCode::NOT_FOUND.into())
}

async fn about() -> About {
    About {}
}

async fn contact() -> Contact {
    Contact {}
}

async fn select_seat(Path((_id, _seat)): Path<(String, String)>) -> Temp {
    Temp {}
}

async fn seating(Path(id): Path<String>) -> Result<Seating> {
    let split_id = id.split_once(':');
    if split_id.is_none() {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    }
    let (_, showtime_id) = split_id.unwrap();

    let query = DB
        .query(r#"SELECT seats as seat FROM type::thing("showtime",$id) SPLIT seat"#)
        .bind(("id", showtime_id))
        .await;
    if query.is_err() {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    }
    let seats: Vec<Seat> = query.unwrap().take(0).expect("nothing in the DB");
    Ok(Seating { id, seats })
}

async fn booking(Path(id): Path<String>) -> Booking {
    Booking { id }
}

async fn movie(Path(_): Path<String>) -> Temp {
    Temp {}
}
