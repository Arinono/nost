use std::time::Duration;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use reqwest::Url;

use crate::{AppState, Error};

fn nonce(length: usize) -> String {
    use rand::Rng;
    let mut text = String::new();
    let mut rng = rand::thread_rng();
    let possible = "abcdefghjkmnpqrstuvwxyz0123456789";
    for _ in 0..length {
        let idx = rng.gen_range(0..possible.len());
        text.push(possible.chars().nth(idx).unwrap());
    }
    text
}

pub async fn authorize(State(app_state): State<AppState>) -> impl IntoResponse {
    let client_id = app_state.env.twitch_client_id.clone();

    let scope = "moderator:read:followers channel:read:subscriptions";
    let state = nonce(30);

    let url = Url::parse(&format!(
        "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        client_id,
        app_state.env.twitch_user_oauth_callback_url,
        scope,
        state,
    )).expect("valid url");

    tracing::info!("Generated URL: {}", url);
    tracing::info!(state = state.as_str(), "Store state");

    app_state
        .retainer
        .insert(state, "".to_string(), Duration::from_secs(300))
        .await;

    Redirect::temporary(url.as_str()).into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    scope: Vec<String>,
    token_type: String,
}

pub async fn callback(
    State(app_state): State<AppState>,
    query: Query<CallbackQuery>,
) -> Result<impl IntoResponse, Error> {
    let code = query.code.clone();
    let state = query.state.clone();
    tracing::info!(code = code, state = state, "Callback hit");

    let saved_state = app_state.retainer.remove(&state).await;
    tracing::info!(
        found_state = if saved_state.is_some() { true } else { false },
        "Retrieved state"
    );

    if saved_state.is_none() {
        return Err(Error::AppError(anyhow::anyhow!("Callback state invalid")));
    }

    let reqwest_client = reqwest::Client::new();
    let params: &[(&str, &str)] = &[
        ("client_id", &app_state.env.twitch_client_id),
        (
            "client_secret",
            &app_state.env.twitch_client_secret.secret_str(),
        ),
        ("code", &code),
        ("grant_type", "authorization_code"),
        (
            "redirect_uri",
            &app_state.env.twitch_user_oauth_callback_url,
        ),
    ];

    let _: TokenResponse = reqwest_client
        .post("https://id.twitch.tv/oauth2/token")
        .form(&params)
        .send()
        .await?
        .json()
        .await?;

    let msg = "App successfully authorized";
    tracing::info!(msg = msg, "Callback success");

    Ok((StatusCode::OK, Html(msg.to_string())))
}
