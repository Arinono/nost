use axum::{extract::State, response::IntoResponse};
use http::StatusCode;

use crate::{airtable::Airtable, AppState};

pub async fn latest_follow(State(state): State<AppState>) -> impl IntoResponse {
    let airtable_client = Airtable::new(
        state.env.airtable_api_token.clone(),
        state.env.airtable_base_id.clone(),
        state.user_cache.clone(),
        state.subgift_cache.clone(),
    );

    match airtable_client.get_most_recent_follow().await {
        Some(follow) => (StatusCode::OK, follow),
        None => (StatusCode::NOT_FOUND, "No follow found".to_owned()),
    }
}

pub async fn latest_subscriber(State(state): State<AppState>) -> impl IntoResponse {
    let airtable_client = Airtable::new(
        state.env.airtable_api_token.clone(),
        state.env.airtable_base_id.clone(),
        state.user_cache.clone(),
        state.subgift_cache.clone(),
    );

    match airtable_client.get_most_recent_subscriber().await {
        Some(sub) => (StatusCode::OK, sub),
        None => (StatusCode::NOT_FOUND, "No subscriber found".to_owned()),
    }
}

pub async fn latest_subgift(State(state): State<AppState>) -> impl IntoResponse {
    let airtable_client = Airtable::new(
        state.env.airtable_api_token.clone(),
        state.env.airtable_base_id.clone(),
        state.user_cache.clone(),
        state.subgift_cache.clone(),
    );

    match airtable_client.get_most_recent_subgift().await {
        Some(subgift) => (StatusCode::OK, subgift),
        None => (StatusCode::NOT_FOUND, "No subgift found".to_owned()),
    }
}
