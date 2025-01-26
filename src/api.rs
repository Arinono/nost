use axum::{extract::State, response::IntoResponse};
use http::StatusCode;
use tables::latests::Latests;

use crate::AppState;

pub async fn latest_follow(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.database.clone();
    let conn = db.conn().unwrap();

    let follower = Latests::get_latest_follower(&conn)
        .await
        .expect("Failed to get latest follower");

    match follower {
        Some(follow) => (StatusCode::OK, follow.name),
        None => (StatusCode::NOT_FOUND, "No follow found".to_owned()),
    }
}

pub async fn latest_subscriber(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.database.clone();
    let conn = db.conn().unwrap();

    let subscriber = Latests::get_latest_subscriber(&conn)
        .await
        .expect("Failed to get latest subscriber");

    match subscriber {
        Some(sub) => (StatusCode::OK, sub.name),
        None => (StatusCode::NOT_FOUND, "No subscriber found".to_owned()),
    }
}

pub async fn latest_subgift(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.database.clone();
    let conn = db.conn().unwrap();

    let subgift = Latests::get_latest_subgift(&conn)
        .await
        .expect("Failed to get latest subgift");

    match subgift {
        Some(subgift) => (StatusCode::OK, subgift.name),
        None => (StatusCode::NOT_FOUND, "No subgift found".to_owned()),
    }
}

pub async fn latest_bits(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.database.clone();
    let conn = db.conn().unwrap();

    let bits = Latests::get_latest_bit(&conn)
        .await
        .expect("Failed to get latest bit");

    match bits {
        Some(bits) => (StatusCode::OK, bits.name),
        None => (StatusCode::NOT_FOUND, "No subgift found".to_owned()),
    }
}
