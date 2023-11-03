use askama::Template;
use axum::{extract::Path, http::StatusCode, response::Result};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::DB;

#[derive(Template)]
#[template(path = "movie_page.html")]
pub struct MovieAbout {
    movie: Movie,
    stars: Vec<Actor>,
    writers: Vec<String>,
    director: String,
    actors: Vec<Actor>,
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
pub struct Actor {
    pub name: String,
    pub role: String,
}
pub async fn movie(Path(id): Path<String>) -> Result<MovieAbout> {
    let Some(split_id) = id.split_once(':') else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let (_, showtime_id) = split_id;
    let Ok(Some(movie)): surrealdb::Result<Option<Movie>> =
        DB.select(("movies", showtime_id)).await
    else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let query = DB
        .query(
            r#"
            SELECT VALUE (
                SELECT (->people.name)[0] AS name, role 
                FROM ->star
            )
            FROM ONLY type::thing("movies", $id)
            "#,
        )
        .query(
            r#"
            SELECT VALUE (
                SELECT VALUE (->people.name)[0] AS name 
                FROM ->writer
            )
            FROM ONLY type::thing("movies", $id)
            "#,
        )
        .query(
            r#"
            SELECT VALUE (
                SELECT VALUE (->people.name)[0]
                FROM ONLY ->director
            )
            FROM ONLY type::thing("movies", $id)
            "#,
        )
        .query(
            r#"
            SELECT VALUE (
                SELECT (->people.name)[0] AS name, role
                FROM ->actor
            )
            FROM ONLY type::thing("movies", $id)
            "#,
        )
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    let Ok(stars) = query.take(0) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    let Ok(writers) = query.take(1) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    let Ok(Some(director)) = query.take(2) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    let Ok(actors) = query.take(3) else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    Ok(MovieAbout {
        movie,
        stars,
        writers,
        director,
        actors,
    })
}
