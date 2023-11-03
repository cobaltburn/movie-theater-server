use crate::DB;
use askama::Template;
use axum::{extract::Path, http::StatusCode, response::Result};
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

pub async fn select_seat(Path((id, seat)): Path<(String, i32)>) -> Result<ConfirmationPage> {
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
    let Ok(Some(movie)) = query.take(0) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Ok(Some(time)) = query.take(1) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    Ok(ConfirmationPage {
        id,
        time,
        seat,
        movie,
    })
}

pub async fn seating(Path(id): Path<String>) -> Result<SeatingPage> {
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
