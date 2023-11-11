use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::Form,
    http::StatusCode,
    response::{Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use regex::Regex;
use serde::Deserialize;
use surrealdb::sql::Thing;

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
pub struct SignUp {
    email: String,
    valid_email: bool,
    account_found: bool,
    valid_password: bool,
}

#[derive(Template)]
#[template(path = "temp.html")]
pub struct Temp {}

#[derive(Deserialize, Debug)]
pub struct Account {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct Record {
    #[allow(dead_code)]
    id: Thing,
}

pub async fn get_login(jar: PrivateCookieJar) -> Response {
    let Some(session) = jar.get("session") else {
        return Login {
            email: String::new(),
            password: String::new(),
            valid_email: true,
            account_found: true,
        }
        .into_response();
    };

    let Ok(Some(_)) = DB
        .select::<Option<Record>>(("sessions", session.value()))
        .await
    else {
        return Login {
            email: String::new(),
            password: String::new(),
            valid_email: true,
            account_found: true,
        }
        .into_response();
    };
    Redirect::to("/account").into_response()
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
            SELECT VALUE id
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

    let Ok(Some(id)) = query.take::<Option<Thing>>(0) else {
        return Login {
            email,
            password,
            valid_email: true,
            account_found: false,
        }
        .into_response();
    };

    let query = DB
        .query(
            r#"            
            BEGIN TRANSACTION;

            LET $new_session = CREATE ONLY sessions;

            RELATE $new_session->account_session->$user 
                SET time = time::now();

            RETURN $new_session.id;

            COMMIT TRANSACTION;
            "#,
        )
        .bind(("user", &id))
        .await;

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let Ok(Some(session)): Result<Option<Thing>, _> = query.take(0) else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };
    let mut jar = jar
        .add(Cookie::new("session", session.id.to_raw().clone()))
        .into_response();
    jar.headers_mut()
        .insert("HX-Redirect", "/".parse().unwrap());
    jar
}

pub async fn sign_up() -> SignUp {
    SignUp {
        email: String::new(),
        valid_email: true,
        valid_password: true,
        account_found: false,
    }
}

pub async fn create_account(
    jar: PrivateCookieJar,
    Form(Account { email, password }): Form<Account>,
) -> Response {
    let valid_email = is_valid_email(&email);
    let valid_password = is_valid_password(&password);
    if !valid_email || !valid_password {
        return SignUp {
            email,
            valid_email,
            valid_password,
            account_found: false,
        }
        .into_response();
    }

    let query = DB
        .query(
            r#"
            BEGIN TRANSACTION;

            LET $user = CREATE ONLY accounts SET email = $email, password = $password;

            LET $new_session = CREATE ONLY sessions;

            RELATE $new_session->account_session->$user 
                SET time = time::now();

            RETURN $new_session.id;

            COMMIT TRANSACTION;
            "#,
        )
        .bind(("email", &email))
        .bind(("password", &password))
        .await;

    let Ok(mut query) = query else {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    };

    let Ok(Some(session)): Result<Option<Thing>, _> = query.take(0) else {
        return SignUp {
            email,
            valid_email,
            valid_password,
            account_found: true,
        }
        .into_response();
    };
    let mut jar = jar
        .add(Cookie::new("session", session.id.to_raw().clone()))
        .into_response();
    jar.headers_mut()
        .insert("HX-Redirect", "/".parse().unwrap());
    jar
}

fn is_valid_email(email: &String) -> bool {
    let email_pattern = Regex::new(r#"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$"#).unwrap();
    email_pattern.is_match(email)
}

fn is_valid_password(password: &String) -> bool {
    !password.is_empty()
}

pub async fn logout(jar: PrivateCookieJar) -> Response {
    println!("test");
    let jar = jar.remove(Cookie::named("session"));
    (jar, Redirect::to("/")).into_response()
}
