use crate::DB;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Form, Path},
    http::StatusCode,
    response::{Redirect, Response, Result},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Template)]
#[template(path = "unavailable.html")]
pub struct Unavailable {}

#[derive(Template)]
#[template(path = "complete.html")]
pub struct Complete {
    pub movie: String,
    pub time: String,
    pub seat: i32,
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
struct Account {
    id: Thing,
    email: String,
}

pub async fn purchase(Path((id, seat)): Path<(String, i32)>) -> Result<PurchasePage> {
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

pub async fn complete_purchase(
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

    let Some(split_id) = id.split_once(':') else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let (_, showtime_id) = split_id;

    let query = DB
        .query(
            r#"
            SELECT * 
            FROM ONLY accounts
            WHERE email = $email
            "#,
        )
        .bind(("email", &email))
        .await;

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let Some(account): Option<Account> = query.take(0).unwrap() else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    // let Some(showtime_seat): Option<ShowtimeSeat> = query.take(0).expect("invalid query index")
    // else {
    //     return Redirect::to("/purchase/unavailable").into_response();
    // };

    //TODO valid that seat is available
    //TODO return hash value for purchase
    let url = format!("/purchase/complete/{}/{}/{}", seat, movie, time);
    Redirect::to((*url).into()).into_response()
}

pub async fn complete(Path((seat, movie, time)): Path<(i32, String, String)>) -> Complete {
    Complete { movie, time, seat }
}

pub async fn unavailable() -> Unavailable {
    Unavailable {}
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
