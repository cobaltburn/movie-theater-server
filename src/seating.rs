use crate::DB;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Form, Path},
    http::StatusCode,
    response::{Redirect, Response, Result},
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

#[derive(Template)]
#[template(path = "times.html")]
pub struct Times {
    pub times: Vec<Time>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Seat {
    pub available: bool,
    pub seat: i32,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ShowInformation {
    day: i32,
    time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Day {
    pub day: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Time {
    id: Thing,
    time: String,
}

pub async fn select_seat(jar: PrivateCookieJar, Path((id, seat)): Path<(String, i32)>) -> Response {
    if let Err(err) = check_session(&jar).await {
        return err;
    }

    let query = DB
        .query(
            r#"
            SELECT (<-showing<-theaters<-playing<-movies.*)[0] AS movie,
            time::format(time, "%k:%M, %a") AS time
            FROM ONLY type::thing("showtime",$id)
            "#,
        )
        .bind(("id", &id))
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

pub async fn seating(Path(id): Path<String>) -> Result<SeatingPage> {
    let Some((_, id)) = id.split_once(':') else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let query = DB
        .query(
            r#"            
            SELECT VALUE ->showtime_seat->seats.* 
            FROM ONLY type::thing("showtime", $id);            
            "#,
        )
        .bind(("id", &id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Ok(mut seats): Result<Vec<Seat>, _> = query.take(0) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    seats.sort_by(|a, b| a.seat.cmp(&b.seat));
    Ok(SeatingPage {
        id: id.to_string(),
        seats,
    })
}

pub async fn times(Path(id): Path<String>, Form(Day { day }): Form<Day>) -> Result<Times> {
    let Some((_, id)) = id.split_once(':') else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    let query = DB
        .query(
            r#" 
            SELECT id, time::format(time, "%k:%M") AS time
            FROM showtime
            WHERE <-showing<-theaters CONTAINS type::thing("theaters", $id) &&
            day = $day
            ORDER BY time
            "#,
        )
        .bind(("id", id))
        .bind(("day", day))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Ok(times): Result<Vec<Time>, _> = query.take(0) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    Ok(Times { times })
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
