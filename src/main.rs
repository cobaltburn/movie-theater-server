use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Form, Path},
    http::StatusCode,
    response::{Redirect, Response, Result},
    routing::{get, get_service, post},
    Router,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
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
#[template(path = "unavailable.html")]
struct Unavailable {}

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
#[template(path = "complete.html")]
struct Complete {
    movie: String,
    time: String,
    seat: i32,
}

#[derive(Template)]
#[template(path = "purchase.html")]
struct PurchasePage {
    id: String,
    time: String,
    seat: i32,
    movie: String,
    card_num: String,
    exp_date: String,
    cvv: String,
    email: String,
    valid_email: bool,
}

impl PurchasePage {
    fn new(id: String, time: String, seat: i32, movie: String) -> PurchasePage {
        PurchasePage {
            id,
            time,
            seat,
            movie,
            card_num: String::new(),
            exp_date: String::new(),
            cvv: String::new(),
            email: String::new(),
            valid_email: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Seat {
    available: bool,
    number: i32,
    id: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    id: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
struct Movie {
    id: Thing,
    name: String,
    genres: Vec<String>,
    runtime: i32,
    tagline: String,
    stars: f32,
    description: String,
    image: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MovieShowTimes {
    id: Thing,
    name: String,
    genres: Vec<String>,
    runtime: i32,
    tagline: String,
    stars: f32,
    description: String,
    image: String,
    times: Vec<Time>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Time {
    id: Thing,
    time: String,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    card_num: String,
    exp_date: String,
    cvv: String,
    email: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShowtimeSeat {
    id: Thing,
    seat: i32,
    time: String,
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
        .route("/purchase/unavailable", get(unavailable))
        .route("/purchase/:id/:seat", get(purchase))
        .route("/purchase/:id/:seat/:movie/:time", post(complete_purchase))
        .route("/purchase/complete/:seat/:movie/:time", get(complete))
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
        .query(r#"SELECT VALUE ->showtime_seat->seats.* FROM ONLY type::thing("showtime",$id)"#)
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Ok(mut seats): Result<Vec<Seat>, _> = query.take(0) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    seats.sort_by(|a, b| a.number.cmp(&b.number));

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

    Ok(PurchasePage::new(id, time, seat, movie))
}

async fn complete_purchase(
    Path((id, seat, movie, time)): Path<(String, i32, String, String)>,
    email: Option<Form<UserInfo>>,
) -> Response {
    let Some(Form(email)) = email else {
        println!("invalid query");
        println!("{:?}", email);
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let UserInfo {
        email,
        card_num,
        exp_date,
        cvv,
    } = email;
    if !valid_email(&email) {
        return PurchasePage {
            id,
            time,
            seat,
            movie,
            card_num,
            exp_date,
            cvv,
            email,
            valid_email: false,
        }
        .into_response();
    }

    let Some(split_id) = id.split_once(':') else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let (_, showtime_id) = split_id;
    let query = DB
        .query(
            r#"
            SELECT id, seats[WHERE seat = $seat AND available = true].seat AS seat, time::format(time, "%k:%M, %e %h") AS time
            FROM type::thing("showtime",$id) 
            SPLIT seat
            "#,
        )
        .bind(("seat", seat))
        .bind(("id", showtime_id))
        .await;

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let Some(showtime_seat): Option<ShowtimeSeat> = query.take(0).expect("invalid query index")
    else {
        return Redirect::to("/purchase/unavailable").into_response();
    };

    //TODO valid that seat is available
    //TODO return hash value for purchase
    let url = format!("/purchase/complete/{}/{}/{}", seat, movie, time);
    Redirect::to((*url).into()).into_response()
}

async fn complete(Path((seat, movie, time)): Path<(i32, String, String)>) -> Complete {
    Complete { movie, time, seat }
}

async fn unavailable() -> Unavailable {
    Unavailable {}
}

async fn booking(Path(id): Path<String>) -> BookingPage {
    BookingPage { id }
}

async fn movie(Path(_): Path<String>) -> Temp {
    Temp {}
}

fn valid_email(email: &String) -> bool {
    let email_pattern =
        regex::Regex::new(r#"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$"#).unwrap();

    // Check if the provided email matches the pattern.
    email_pattern.is_match(email)
}
