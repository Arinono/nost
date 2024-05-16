use axum::{
    extract::{rejection::BytesRejection, FromRequest, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use hmac::{digest::InvalidLength, Hmac, Mac};
use sha2::Sha256;

use crate::AppState;

const TWI_MSG_ID: &str = "Twitch-Eventsub-Message-Id";
const TWI_MSG_SIG: &str = "Twitch-Eventsub-Message-Signature";
const TWI_MSG_TIM: &str = "Twitch-Eventsub-Message-Timestamp";

#[derive(Debug, Clone)]
struct EventSubPayload {}

pub async fn eventsub(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(event): Json<EventSubPayload>,
) -> impl IntoResponse {
    let secret = &state.env.event_sub_secret;
    let msg_id = headers.get(TWI_MSG_ID).expect("No message ID").to_str();
    let msg_ts = headers
        .get(TWI_MSG_TIM)
        .expect("No message timestamp")
        .to_str();

    (StatusCode::OK, Html("EventSub".to_string()))
}
