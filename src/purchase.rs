use crate::DB;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Form, Path},
    http::StatusCode,
    response::{Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use qrcode::render::svg;
use qrcode::QrCode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};

#[derive(Template)]
#[template(path = "unavailable.html")]
pub struct Unavailable {}

#[derive(Template)]
#[template(path = "complete.html")]
pub struct Complete {
    pub movie: String,
    pub time: String,
    pub seat: i32,
    pub ticket: Id,
    pub svg: String,
}

#[derive(Template)]
#[template(path = "purchase.html")]
pub struct PurchasePage {
    pub id: String,
    pub time: String,
    pub seat: i32,
    pub movie: String,
    pub card_num: String,
    pub exp_date: String,
    pub cvv: String,
    pub email: String,
    pub valid_card_num: bool,
    pub valid_exp: bool,
    pub valid_cvv: bool,
    pub valid_email: bool,
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
            valid_card_num: true,
            valid_cvv: true,
            valid_exp: true,
            valid_email: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub card_num: String,
    pub exp_date: String,
    pub cvv: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShowtimeSeat {
    pub id: Thing,
    pub seat: i32,
    pub time: String,
}

#[derive(Debug, Deserialize)]
struct MovieTime {
    movie: String,
    time: String,
}

#[derive(Deserialize)]
struct Record {
    #[allow(dead_code)]
    id: Thing,
}

pub async fn purchase(jar: PrivateCookieJar, Path((id, seat)): Path<(String, i32)>) -> Response {
    if let Err(err) = check_session(&jar).await {
        return err;
    }
    let Some((_, showtime_id)) = id.split_once(':') else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let query = DB
        .query(
            r#"
            SELECT (<-showing<-theaters<-playing<-movies.name)[0] AS movie, 
            time::format(time, "%k:%M") AS time
            FROM ONLY type::thing("showtime", $id)
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

    PurchasePage::new(id, time, seat, movie).into_response()
}

pub async fn complete_purchase(
    Path((id, seat, movie, time)): Path<(String, i32, String, String)>,
    Form(UserInfo {
        card_num,
        exp_date,
        cvv,
        email,
    }): Form<UserInfo>,
) -> Response {
    let valid_email = is_valid_email(&email);
    let valid_card_num = is_valid_card_number(&card_num);
    let valid_cvv = is_valid_cvv(&cvv);
    let valid_exp = is_valid_exp(&exp_date);
    if !valid_email || !valid_card_num || !valid_cvv || !valid_exp {
        return PurchasePage {
            id,
            time,
            seat,
            movie,
            card_num,
            exp_date,
            cvv,
            email,
            valid_card_num,
            valid_cvv,
            valid_exp,
            valid_email,
        }
        .into_response();
    }

    let Some((_, showtime_id)) = id.split_once(':') else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let query = DB
        .query(
            r#"
            SELECT VALUE id 
            FROM ONLY accounts
            WHERE email = $email
            "#,
        )
        .bind(("email", &email))
        .await;

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let (query, index) = if let Ok(Some(user_id)) = query.take(0) {
        let user_id: Thing = user_id;
        (DB
            .query(
                r#"
                BEGIN TRANSACTION;

                LET $seat = SELECT VALUE ->showtime_seat->(seats WHERE number = $seat_num AND available = true).*
                FROM ONLY type::thing("showtime", $id);

                UPDATE $seat SET available = false;

                RELATE ONLY $user->purchase->$seat SET time = time::now(), card_number = $card_number, exp_date = $exp_date RETURN VALUE id;

                COMMIT TRANSACTION                
                "#,
            )
            .bind(("seat_num", seat))
            .bind(("id", &showtime_id))
            .bind(("user", &user_id))
            .bind(("card_number", &card_num))
            .bind(("exp_date", &exp_date))
            .await, 2)
    } else {
        (DB
            .query(
                r#"
                BEGIN TRANSACTION;

                LET $user = CREATE accounts SET email = $email;

                LET $seat = SELECT VALUE ->showtime_seat->(seats WHERE number = $seat_num AND available = true).*
                FROM ONLY type::thing("showtime", $id);

                UPDATE $seat SET available = false;

                RELATE ONLY $user->purchase->$seat SET time = time::now(), card_number = $card_number, exp_date = $exp_date RETURN VALUE id;

                COMMIT TRANSACTION                
                "#,
            )
            .bind(("email", &email))
            .bind(("seat_num", seat))
            .bind(("id", &showtime_id))
            .bind(("card_number", &card_num))
            .bind(("exp_date", &exp_date))
            .await, 3)
    };

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let Ok(result) = query.take(index) else {
        return Unavailable {}.into_response();
    };

    let Some(ticket): Option<Thing> = result else {
        return Unavailable {}.into_response();
    };

    let code = QrCode::new(ticket.id.to_string().as_bytes()).unwrap();
    let svg = code
        .render()
        .min_dimensions(400, 400)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    Complete {
        movie,
        time,
        seat,
        ticket: ticket.id,
        svg,
    }
    .into_response()
}

fn is_valid_email(email: &String) -> bool {
    let email_pattern = Regex::new(r#"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$"#).unwrap();
    email_pattern.is_match(email)
}

fn is_valid_exp(exp: &String) -> bool {
    let exp_pattern = Regex::new(r"^(0[1-9]|1[0-2])/\d{2}$").unwrap();
    exp_pattern.is_match(exp)
}

fn is_valid_card_number(card_num: &String) -> bool {
    if card_num.len() != 16 {
        return false;
    }
    for ch in card_num.chars() {
        if !ch.is_numeric() {
            return false;
        }
    }
    true
}

fn is_valid_cvv(cvv: &String) -> bool {
    if cvv.len() != 3 {
        return false;
    }
    for ch in cvv.chars() {
        if !ch.is_numeric() {
            return false;
        }
    }
    true
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
