use crate::DB;
use askama::Template;
use axum::{http::StatusCode, response::Result};
use axum_extra::extract::PrivateCookieJar;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index {
    logged_in: bool,
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomePage {
    pub movies: Vec<Movie>,
}

#[derive(Template)]
#[template(path = "contact.html")]
pub struct ContactPage {}

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutPage {}

#[derive(Template)]
#[template(path = "footer.html")]
pub struct Footer {}

#[derive(Template)]
#[template(path = "showtime.html")]
pub struct ShowtimePage {
    pub movies: Vec<MovieShowTimes>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Movie {
    pub id: Thing,
    pub name: String,
    pub genres: Vec<String>,
    pub runtime: i32,
    pub tagline: String,
    pub stars: f32,
    pub description: String,
    pub image: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MovieShowTimes {
    pub id: Thing,
    pub name: String,
    pub genres: Vec<String>,
    pub runtime: i32,
    pub tagline: String,
    pub stars: f32,
    pub description: String,
    pub image: String,
    pub times: Vec<Time>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Time {
    pub id: Thing,
    pub time: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    id: Thing,
}

pub async fn index(jar: PrivateCookieJar) -> Index {
    let Some(session) = jar.get("session") else {
        return Index { logged_in: false };
    };
    let Ok(Some(_)) = DB
        .select::<Option<Record>>(("sessions", session.value()))
        .await
    else {
        return Index { logged_in: false };
    };
    Index { logged_in: true }
}

pub async fn home() -> Result<HomePage> {
    let Ok(movies) = DB.select("movies").await else {
        return Err(StatusCode::NOT_FOUND.into());
    };
    Ok(HomePage { movies })
}
pub async fn about() -> AboutPage {
    AboutPage {}
}

pub async fn contact() -> ContactPage {
    ContactPage {}
}

pub async fn showtimes() -> Result<ShowtimePage> {
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

pub async fn footer() -> Footer {
    Footer {}
}
