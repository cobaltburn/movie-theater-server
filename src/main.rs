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
struct HomePage {
    movies: Vec<Movie>,
}

#[derive(Template)]
#[template(path = "contact.html")]
struct ContactPage {}

#[derive(Template)]
#[template(path = "about.html")]
struct AboutPage {}

#[derive(Template)]
#[template(path = "showtime.html")]
struct ShowtimePage {
    movies: Vec<MovieShowTimes>,
}

#[derive(Template)]
#[template(path = "booking.html")]
struct BookingPage {
    id: String,
}

#[derive(Template)]
#[template(path = "seating.html")]
struct SeatingPage {
    id: String,
    seats: Vec<Seat>,
}

#[derive(Template)]
#[template(path = "seat_confirmation.html")]
struct ConfirmationPage {
    id: String,
    time: String,
    seat: i32,
    movie: Movie,
}

#[derive(Template)]
#[template(path = "purchase.html")]
struct PurchasePage {
    id: String,
    time: String,
    seat: i32,
    movie: String,
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
        .route("/about", get(about))
        .route("/contact", get(contact))
        .route("/showtimes", get(showtimes))
        .route("/movie/:id", get(movie))
        .route("/booking/:id", get(booking))
        .route("/seating/:id", get(seating))
        .route("/seating/:id/:seat", get(select_seat))
        .route("/purchase/:id/:seat", get(purchase))
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

async fn home() -> Result<HomePage> {
    let Ok(movies) = DB.select("movies").await else {
        return Err(StatusCode::NOT_FOUND.into());
    };
    Ok(HomePage { movies })
}

async fn showtimes() -> Result<ShowtimePage> {
    let query = DB
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
    let Ok(mut qeury) = query else {
        return Err(StatusCode::NOT_FOUND.into());
    };
    let Ok(movies) = qeury.take(0) else {
        return Err(StatusCode::NOT_FOUND.into());
    };
    Ok(ShowtimePage { movies })
}

async fn about() -> AboutPage {
    AboutPage {}
}

async fn contact() -> ContactPage {
    ContactPage {}
}

async fn select_seat(Path((id, seat)): Path<(String, i32)>) -> Result<ConfirmationPage> {
    let split_id = id.split_once(':');
    if split_id.is_none() {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    }

    let (_, showtime_id) = split_id.unwrap();
    let query = DB
        .query(
            r#"
            SELECT VALUE <-showing<-theaters<-playing<-movies.*
            FROM ONLY type::thing("showtime",$id)
            "#,
        )
        .query(
            r#"
            SELECT VALUE time::format(time, "%k:%M")
            FROM ONLY type::thing("showtime", $id)
            "#,
        )
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Some(movie) = query.take(0).expect("invalid-query index") else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Some(time) = query.take(1).expect("invalid-query index") else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    Ok(ConfirmationPage {
        id,
        time,
        seat,
        movie,
    })
}

async fn seating(Path(id): Path<String>) -> Result<SeatingPage> {
    let Some(split_id) = id.split_once(':') else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let (_, showtime_id) = split_id;

    let query = DB
        .query(r#"SELECT VALUE seats FROM type::thing("showtime",$id)"#)
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Some(seats) = query.take(0).expect("invalid-query index") else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    Ok(SeatingPage { id, seats })
}

async fn purchase(Path((id, seat)): Path<(String, i32)>) -> Result<PurchasePage> {
    let Some(split_id) = id.split_once(':') else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let (_, showtime_id) = split_id;
    let query = DB
        .query(
            r#"
            SELECT VALUE time::format(time, "%k:%M, %e %h")
            FROM ONLY type::thing("showtime", $id)
            "#,
        )
        .query(
            r#"
            SELECT VALUE <-showing<-theaters<-playing<-movies.name
            FROM ONLY type::thing("showtime",$id)
            "#,
        )
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Some(time) = query.take(0).expect("invalid query index") else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Some(movie) = query.take(1).expect("invalid query index") else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    Ok(PurchasePage {
        id,
        time,
        seat,
        movie,
    })
}

async fn booking(Path(id): Path<String>) -> BookingPage {
    BookingPage { id }
}

async fn movie(Path(_): Path<String>) -> Temp {
    Temp {}
}
