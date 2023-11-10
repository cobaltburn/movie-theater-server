use askama::Template;
use askama_axum::IntoResponse;
use axum::{extract::Form, http::StatusCode, response::Response};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use regex::Regex;
use serde::Deserialize;

use crate::DB;

#[derive(Template)]
#[template(path = "login.html")]
pub struct Login {
    email: String,
    password: String,
    valid_email: bool,
    account_found: bool,
}

#[derive(Template)]
#[template(path = "sign_up.html")]
pub struct SignUp {}

#[derive(Template)]
#[template(path = "temp.html")]
pub struct Temp {}

#[derive(Deserialize)]
pub struct Account {
    email: String,
    password: String,
}

pub async fn get_login() -> Login {
    Login {
        email: String::new(),
        password: String::new(),
        valid_email: true,
        account_found: true,
    }
}

pub async fn post_login(
    jar: PrivateCookieJar,
    Form(Account { email, password }): Form<Account>,
) -> Response {
    let valid_email = is_valid_email(&email);
    if !valid_email {
        return Login {
            email,
            password,
            valid_email,
            account_found: true,
        }
        .into_response();
    }
    let query = DB
        .query(
            r#"
            SELECT * 
            FROM ONLY accounts
            WHERE email = $email AND password = $password
            "#,
        )
        .bind(("email", &email))
        .bind(("password", &password))
        .await;

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let Ok(result) = query.take(0) else {
        return Login {
            email,
            password,
            valid_email: true,
            account_found: false,
        }
        .into_response();
    };

    let Some(account): Option<Account> = result else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    //TODO enable session
    let query = DB.query("").await;

    (
        jar,
        Login {
            email,
            password,
            valid_email,
            account_found: true,
        },
    )
        .into_response()
}

pub async fn sign_up() -> SignUp {
    SignUp {}
}

fn is_valid_email(email: &String) -> bool {
    let email_pattern = Regex::new(r#"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$"#).unwrap();
    email_pattern.is_match(email)
}
