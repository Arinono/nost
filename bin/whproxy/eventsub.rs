use axum::{
    body::HttpBody,
    extract::State,
    http::{self, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use twitch_api::eventsub::{channel::ChannelFollowV2Payload, event::Event};

use crate::{
    airtable::{Airtable, User},
    discord::DiscordNotifier,
    AppState,
};

const TWI_MSG_ID: &str = "Twitch-Eventsub-Message-Id";
// const TWI_MSG_SIG: &str = "Twitch-Eventsub-Message-Signature";
// const TWI_MSG_TIM: &str = "Twitch-Eventsub-Message-Timestamp";

const MAX_ALLOWED_RESPONSE_SIZE: u64 = 64 * 1024;

pub async fn eventsub(
    State(app_state): State<AppState>,
    req: http::Request<axum::body::Body>,
) -> impl IntoResponse {
    let secret = &app_state.env.event_sub_secret;
    let (parts, body) = req.into_parts();

    let response_content_length = match body.size_hint().upper() {
        Some(v) => v,
        None => MAX_ALLOWED_RESPONSE_SIZE + 1,
    };

    // just in case
    let body = if response_content_length < MAX_ALLOWED_RESPONSE_SIZE {
        axum::body::to_bytes(body, response_content_length as usize)
            .await
            .unwrap()
    } else {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            "Payload too large".to_string(),
        );
    };

    let request = http::Request::from_parts(parts, &*body);

    tracing::debug!("got event {}", std::str::from_utf8(request.body()).unwrap());
    tracing::debug!("got event headers {:?}", request.headers());

    if !Event::verify_payload(&request, secret.secret()) {
        return (StatusCode::BAD_REQUEST, "Invalid signature".to_string());
    }

    if let Some(id) = request.headers().get(TWI_MSG_ID) {
        let id_string = String::from(id.to_str().unwrap());
        if app_state.retainer.get(&id_string).await.is_none() {
            app_state
                .retainer
                .insert(id_string.clone(), String::new(), 400)
                .await;
        } else {
            tracing::debug!("got already seen event");
            return (StatusCode::OK, "".to_string());
        }
    }

    // Event is verified, now do stuff.
    let event = Event::parse_http(&request).unwrap();
    //let event = Event::parse(std::str::from_utf8(request.body()).unwrap()).unwrap();
    tracing::info_span!("valid_event", event=?event);
    tracing::info!("got event!");

    if let Some(ver) = event.get_verification_request() {
        tracing::info!("subscription was verified");
        return (StatusCode::OK, ver.challenge.clone());
    }

    if event.is_revocation() {
        tracing::info!("subscription was revoked");
        return (StatusCode::OK, "".to_string());
    }

    use twitch_api::eventsub::{Message as M, Payload as P};

    let discord =
        DiscordNotifier::new(app_state.env.discord_webhook_url.secret_str().to_owned()).await;
    let airtable = Airtable::new(
        app_state.env.airtable_api_token.clone(),
        app_state.env.airtable_base_id.clone(),
        app_state.airtable.clone(),
    );

    match event {
        Event::ChannelFollowV2(P {
            message:
                M::Notification(ChannelFollowV2Payload {
                    user_name, user_id, ..
                }),
            ..
        }) => {
            tracing::info!("got follow event from {} ({})", user_name, user_id);

            tokio::spawn(async move {
                discord.new_follower(user_name.to_string()).await;
                let record = airtable.get_user_by_twitch_id(user_id.to_string()).await;

                match record {
                    None => {
                        let new_user = User::builder()
                            .twitch_id(user_id.to_string())
                            .display_name(user_name.to_string())
                            .followed_at(Utc::now())
                            .build();

                        let _ = airtable
                            .create_user(new_user)
                            .await
                            .expect("Failed to create user");
                    }
                    Some(record) => {
                        let mut update_record = record.clone();
                        update_record.fields.follower_since = Some(Utc::now().to_rfc3339());

                        let _ = airtable
                            .update_user(update_record)
                            .await
                            .expect("Failed to update user");
                    }
                }
            });

            return (StatusCode::OK, "EventSub".to_string());
        }
        _ => {}
    }

    (StatusCode::OK, "EventSub".to_string())
}
