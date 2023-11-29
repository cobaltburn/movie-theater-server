use askama::Template;
use askama_axum::IntoResponse;
use axum::{extract::Form, http::StatusCode, response::Response};
use axum_extra::extract::cookie::PrivateCookieJar;
use qrcode::render::svg;
use qrcode::QrCode;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::DB;

#[derive(Template)]
#[template(path = "tickets.html")]
pub struct Tickets {
    tickets: Vec<TicketInfo>,
}

#[derive(Template)]
#[template(path = "search_results.html")]
pub struct SearchResults {
    tickets: Vec<TicketInfo>,
}

#[derive(Deserialize)]
pub struct Ticket {
    movie: String,
    time: String,
    seat: i32,
    id: Thing,
}

#[derive(Deserialize)]
pub struct TicketInfo {
    movie: String,
    time: String,
    seat: i32,
    id: String,
    svg: String,
}

impl TicketInfo {
    fn from_ticket(
        Ticket {
            movie,
            time,
            seat,
            id,
        }: Ticket,
    ) -> Self {
        let id = id.id.to_raw();
        let code = QrCode::new(id.as_bytes()).unwrap();
        let svg = code
            .render()
            .min_dimensions(400, 400)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build();
        TicketInfo {
            movie,
            time,
            seat,
            id,
            svg,
        }
    }
}

#[derive(Deserialize)]
pub struct Query {
    query: String,
}

pub async fn search_tickets(jar: PrivateCookieJar, Form(Query { query }): Form<Query>) -> Response {
    let Some(session) = jar.get("session") else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let query = DB
        .query(
            r#"
            SELECT seat, 
            (<-purchase[0].id) AS id, 
            (<-showtime_seat<-showtime<-showing<-theaters<-playing<-movies.name)[0] AS movie, 
            time::format((<-showtime_seat<-showtime.time)[0], "%k:%M, %x") AS time
            FROM (
                SELECT VALUE ->account_session->accounts->purchase->seats 
                FROM ONLY type::thing("sessions", $id)
            )
            WHERE string::contains(string::lowercase((<-showtime_seat<-showtime<-showing<-theaters<-playing<-movies.name)[0]), string::lowercase($query)) ||
            string::contains(time::format((<-showtime_seat<-showtime.time)[0], "%k:%M, %x"), $query) ||
            type::string(seat) = $query ||
            string::contains(type::string(<-purchase[0].id), $query)
            ORDER BY time;
            "#,
        )
        .bind(("id", session.value()))
        .bind(("query", query))
        .await;
    let Ok(mut query) = query else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(tickets) = query.take::<Vec<Ticket>>(0) else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let tickets = tickets
        .into_iter()
        .map(|ticket| TicketInfo::from_ticket(ticket))
        .collect();
    SearchResults { tickets }.into_response()
}

pub async fn tickets(jar: PrivateCookieJar) -> Response {
    let Some(session) = jar.get("session") else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let query = DB
        .query(
            r#"
            SELECT seat, 
            (<-purchase[0].id) AS id, 
            (<-showtime_seat<-showtime<-showing<-theaters<-playing<-movies.name)[0] AS movie, 
            time::format((<-showtime_seat<-showtime.time)[0], "%k:%M, %x") AS time
            FROM (
                SELECT VALUE ->account_session->accounts->purchase->seats 
                FROM ONLY type::thing("sessions", $id)
            )
            ORDER BY time;
            "#,
        )
        .bind(("id", session.value()))
        .await;
    let Ok(mut query) = query else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(tickets) = query.take::<Vec<Ticket>>(0) else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let tickets = tickets
        .into_iter()
        .map(|ticket| TicketInfo::from_ticket(ticket))
        .collect();
    Tickets { tickets }.into_response()
}
