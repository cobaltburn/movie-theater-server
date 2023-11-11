use crate::DB;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::Path,
    http::StatusCode,
    response::{Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Template)]
#[template(path = "seat_confirmation.html")]
pub struct ConfirmationPage {
    pub id: String,
    pub time: String,
    pub seat: i32,
    pub movie: Movie,
}
#[derive(Template)]
#[template(path = "seating.html")]
pub struct SeatingPage {
    pub id: String,
    pub seats: Vec<Seat>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Seat {
    pub available: bool,
    pub number: i32,
    pub id: Thing,
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

#[derive(Debug, Deserialize)]
struct MovieTime {
    movie: Movie,
    time: String,
}

#[derive(Deserialize)]
struct Record {
    #[allow(dead_code)]
    id: Thing,
}

pub async fn select_seat(jar: PrivateCookieJar, Path((id, seat)): Path<(String, i32)>) -> Response {
    if let Err(err) = check_session(&jar).await {
        return err;
    }
    let Some((_, showtime_id)) = id.split_once(':') else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let query = DB
        .query(
            r#"
            SELECT (<-showing<-theaters<-playing<-movies.*)[0] AS movie,
            time::format(time, "%k:%M") AS time
            FROM ONLY type::thing("showtime",$id)
            "#,
        )
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let Ok(Some(MovieTime { movie, time })) = query.take(0) else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    ConfirmationPage {
        id,
        time,
        seat,
        movie,
    }
    .into_response()
}

pub async fn seating(jar: PrivateCookieJar, Path(id): Path<String>) -> Response {
    if let Err(err) = check_session(&jar).await {
        return err;
    }
    let Some((_, showtime_id)) = id.split_once(':') else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let query = DB
        .query(r#"SELECT VALUE ->showtime_seat->seats.* FROM ONLY type::thing("showtime",$id)"#)
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let Ok(mut seats): Result<Vec<Seat>, _> = query.take(0) else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    seats.sort_by(|a, b| a.number.cmp(&b.number));

    SeatingPage { id, seats }.into_response()
}

async fn check_session(jar: &PrivateCookieJar) -> Result<&PrivateCookieJar, Response> {
    let Some(session) = jar.get("session") else {
        return Err(Redirect::to("/login").into_response());
    };
    let Ok(Some(_)) = DB
        .select::<Option<Record>>(("sessions", session.value()))
        .await
    else {
        return Err(Redirect::to("/login").into_response());
    };
    Ok(jar)
}
