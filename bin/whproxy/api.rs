use axum::{extract::State, response::IntoResponse};
use http::StatusCode;

use crate::{airtable::Airtable, AppState};

pub async fn latest_follow(State(state): State<AppState>) -> impl IntoResponse {
    let airtable_client = Airtable::new(
        state.env.airtable_api_token.clone(),
        state.env.airtable_base_id.clone(),
        state.airtable.clone(),
    );

    if let Some(follow) = airtable_client.get_most_recent_follow().await {
        (StatusCode::OK, follow)
    } else {
        (StatusCode::NOT_FOUND, "No follow found".to_owned())
    }
}
