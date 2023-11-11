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

#[derive(Debug, Deserialize)]
struct Cast {
    actors: Vec<Actor>,
    stars: Vec<Actor>,
    writers: Vec<String>,
    director: String,
}

pub async fn movie(Path(id): Path<String>) -> Result<MovieAbout> {
    let Some((_, showtime_id)) = id.split_once(':') else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let Ok(Some(movie)): surrealdb::Result<Option<Movie>> =
        DB.select(("movies", showtime_id)).await
    else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };
    let query = DB
        .query(
            r#"
            SELECT (SELECT (->people.name)[0] AS name, role FROM ->star) AS stars, 
            (SELECT VALUE (->people.name)[0] AS name FROM ->writer) AS writers, 
            (SELECT VALUE (->people.name)[0] FROM ONLY ->director) AS director, 
            (SELECT (->people.name)[0] AS name, role FROM ->actor) AS actors
            FROM ONLY type::thing("movies", $id)
            "#,
        )
        .bind(("id", showtime_id))
        .await;
    let Ok(mut query) = query else {
        return Err(StatusCode::NOT_ACCEPTABLE.into());
    };

    let Ok(Some(Cast {
        actors,
        stars,
        writers,
        director,
    })) = query.take(0)
    else {
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
