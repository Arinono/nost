use std::str::FromStr;

use axum::{
    body::HttpBody,
    extract::State,
    http::{self, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use twitch_api::eventsub::{
    channel::{
        ChannelFollowV2Payload, ChannelSubscribeV1Payload, ChannelSubscriptionEndV1Payload,
        ChannelSubscriptionGiftV1Payload,
    },
    event::Event,
};
use twitch_types::Nickname;

use crate::{
    airtable::Airtable,
    discord::DiscordNotifier,
    models::{self, misc::SubTier, subgift::Subgift, user::User},
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
    let ack: (StatusCode, String) = (StatusCode::OK, "EventSub".to_string());

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
        app_state.user_cache.clone(),
        app_state.subgift_cache.clone(),
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
                discord.new_follower(&user_name.to_string()).await;
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

            return ack;
        }
        Event::ChannelSubscribeV1(P {
            message:
                M::Notification(ChannelSubscribeV1Payload {
                    tier,
                    user_id,
                    user_name,
                    ..
                }),
            ..
        }) => {
            let tier = models::misc::SubTier::from(tier);
            tracing::info!(
                "got sub event from {} ({}) tier {}",
                user_name,
                user_id,
                tier,
            );

            tokio::spawn(async move {
                discord.new_subscriber(&user_name.to_string()).await;
                let record = airtable.get_user_by_twitch_id(user_id.to_string()).await;

                match record {
                    None => {
                        let new_user = User::builder()
                            .twitch_id(user_id.to_string())
                            .display_name(user_name.to_string())
                            .subscribed_at(Utc::now())
                            .subscription_tier(tier)
                            .build();

                        let _ = airtable
                            .create_user(new_user)
                            .await
                            .expect("Failed to create user");
                    }
                    Some(record) => {
                        let mut update_user = record.clone();
                        update_user.fields.subscriber_since = Some(Utc::now().to_rfc3339());
                        update_user.fields.subscription_tier = Some(tier);

                        let _ = airtable
                            .update_user(update_user)
                            .await
                            .expect("Failed to update user");
                    }
                }
            });

            return ack;
        }
        Event::ChannelSubscriptionEndV1(P {
            message:
                M::Notification(ChannelSubscriptionEndV1Payload {
                    user_id, user_name, ..
                }),
            ..
        }) => {
            tracing::info!("got sub end event from {} ({})", user_name, user_id);

            tokio::spawn(async move {
                let record = airtable.get_user_by_twitch_id(user_id.to_string()).await;

                match record {
                    None => {
                        tracing::warn!("got sub end event for unknown user");
                        let new_user = User::builder()
                            .twitch_id(user_id.to_string())
                            .display_name(user_name.to_string())
                            .build();

                        let _ = airtable
                            .create_user(new_user)
                            .await
                            .expect("Failed to create user");
                    }
                    Some(record) => {
                        let mut update_user = record.clone();
                        update_user.fields.subscriber_since = None;
                        update_user.fields.subscription_tier = None;

                        let _ = airtable
                            .update_user(update_user)
                            .await
                            .expect("Failed to update user");
                    }
                }
            });

            return ack;
        }
        Event::ChannelSubscriptionGiftV1(P {
            message:
                M::Notification(ChannelSubscriptionGiftV1Payload {
                    tier,
                    is_anonymous,
                    cumulative_total,
                    total,
                    user_id: twitch_id,
                    user_name,
                    ..
                }),
            ..
        }) => {
            let username = match is_anonymous {
                true => "Anonymous".to_string(),
                false => user_name
                    .unwrap_or(Nickname::from("Anonymouse"))
                    .to_string(),
            };
            let tier = SubTier::from(tier);
            let total = if total > 0 { total as usize } else { 0 };

            discord.subgift(&username, total, &tier).await;

            let user = match twitch_id {
                Some(id) => airtable.get_user_by_twitch_id(id.to_string()).await,
                None => None,
            };
            let user_id = match user.clone() {
                Some(user) => Some(user.id),
                None => None,
            };

            let subgift = Subgift::builder()
                .user_id(user_id.clone())
                .display_name(if user_id.is_none() {
                    None
                } else {
                    Some(username)
                })
                .number(total)
                .tier(tier)
                .build();

            let record_id = airtable
                .create_subgift(subgift)
                .await
                .expect("Failed to create subgift");

            if !is_anonymous && user.is_some() {
                let mut update_user = user.clone().expect("User is None");
                if update_user.fields.subgifts.is_none() {
                    update_user.fields.subgifts = Some(vec![record_id]);
                } else {
                    update_user
                        .fields
                        .subgifts
                        .as_mut()
                        .unwrap()
                        .push(record_id);
                }
                if let Some(cumulative_total) = cumulative_total {
                    update_user.fields.subgift_total = if cumulative_total > 0 {
                        Some(cumulative_total as usize)
                    } else {
                        None
                    };
                }

                let _ = airtable
                    .update_user(update_user)
                    .await
                    .expect("Failed to update user");
            }

            return ack;
        }
        _ => {}
    }

    (StatusCode::OK, "EventSub".to_string())
}
