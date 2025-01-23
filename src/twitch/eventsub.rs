use axum::{
    body::HttpBody,
    extract::State,
    http::{self, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use twitch_api::eventsub::{
    channel::{
        ChannelCheerV1Payload, ChannelFollowV2Payload, ChannelSubscribeV1Payload,
        ChannelSubscriptionEndV1Payload, ChannelSubscriptionGiftV1Payload,
    },
    Event,
};
use twitch_types::DisplayName;

use crate::{
    airtable::Airtable,
    discord::DiscordNotifier,
    models::{self, misc::SubTier, subgift::Subgift, user::User},
    AppState,
};

const TWI_MSG_ID: &str = "Twitch-Eventsub-Message-Id";

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

    tracing::info!("got event {}", std::str::from_utf8(request.body()).unwrap());
    tracing::info!("got event headers {:?}", request.headers());

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
            tracing::info!("got already seen event");
            return (StatusCode::OK, "".to_string());
        }
    }

    let event = Event::parse_http(&request).unwrap();
    tracing::info_span!("valid_event", event=?event);

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
        app_state.bits_cache.clone(),
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
                        update_record.fields.display_name = user_name.to_string();

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
                discord.new_subscriber(&user_name.to_string(), &tier).await;
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
                        update_user.fields.display_name = user_name.to_string();

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
                        update_user.fields.display_name = user_name.to_string();

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
                    .clone()
                    .unwrap_or(DisplayName::from("Anonymous"))
                    .to_string(),
            };
            let tier = SubTier::from(tier);
            let total = if total > 0 { total as usize } else { 0 };
            let cumulative_total = cumulative_total.map(|v| v as usize);

            tracing::info!(
                "got sub gift event from {} tier {} total {} (cumulative total: {:?})",
                username,
                tier,
                total,
                cumulative_total,
            );
            discord.subgift(&username, total, &tier).await;

            tokio::spawn(async move {
                let user = match &twitch_id {
                    Some(id) => airtable.get_user_by_twitch_id(id.to_string()).await,
                    None => None,
                };

                if is_anonymous {
                    let subgift = Subgift::builder()
                        .user_id(None)
                        .display_name(Some(username))
                        .number(total)
                        .tier(tier)
                        .build();

                    let _ = airtable
                        .create_subgift(subgift)
                        .await
                        .expect("Failed to create subgift");

                    return;
                }

                match user {
                    None => {
                        let twitch_id_s = match twitch_id {
                            Some(id) => id.to_string(),
                            None => "".to_string(),
                        };
                        let mut new_user_b = User::builder()
                            .twitch_id(twitch_id_s)
                            .display_name(username.clone());

                        if let Some(cumulative_total) = cumulative_total {
                            new_user_b = new_user_b.subgift_total(cumulative_total as usize);
                        }

                        let new_user = new_user_b.build();

                        let user_record_id = Some(
                            airtable
                                .create_user(new_user)
                                .await
                                .expect("Failed to create user"),
                        );

                        let subgift = Subgift::builder()
                            .user_id(user_record_id.clone())
                            .display_name(Some(username))
                            .number(total)
                            .tier(tier)
                            .build();

                        let _ = airtable
                            .create_subgift(subgift)
                            .await
                            .expect("Failed to create subgift");
                    }
                    Some(user) => {
                        let mut update_user = user.clone();
                        if let Some(cumulative_total) = cumulative_total {
                            if cumulative_total > user.fields.subgift_total.unwrap_or(0) {
                                update_user.fields.subgift_total = Some(cumulative_total as usize);
                            }
                        }
                        if let Some(user_name) = user_name {
                            update_user.fields.display_name = user_name.to_string();
                        }

                        let _ = airtable
                            .update_user(update_user)
                            .await
                            .expect("Failed to update user");

                        let subgift = Subgift::builder()
                            .user_id(Some(user.id.clone()))
                            .display_name(Some(user.fields.display_name))
                            .number(total)
                            .tier(tier)
                            .build();

                        let _ = airtable
                            .create_subgift(subgift)
                            .await
                            .expect("Failed to create subgift");
                    }
                }
            });

            return ack;
        }
        Event::ChannelCheerV1(P {
            message:
                M::Notification(ChannelCheerV1Payload {
                    user_id: twitch_id,
                    user_name,
                    bits,
                    message,
                    is_anonymous,
                    ..
                }),
            ..
        }) => {
            let username = match is_anonymous {
                true => "Anonymous".to_string(),
                false => user_name
                    .unwrap_or(DisplayName::from("Anonymous"))
                    .to_string(),
            };
            let number = if bits > 0 { bits as usize } else { 0 };

            tracing::info!(
                "got bits event from {} bits {} message {}",
                username,
                number,
                message,
            );
            discord.bits(&username, number, &message).await;

            tokio::spawn(async move {
                let user = match &twitch_id {
                    Some(id) => airtable.get_user_by_twitch_id(id.to_string()).await,
                    None => None,
                };

                if is_anonymous {
                    let bits = models::bits::Bits::builder()
                        .user_id(None)
                        .display_name(Some(username))
                        .number(number)
                        .message(Some(message))
                        .build();

                    let _ = airtable
                        .create_bits(bits)
                        .await
                        .expect("Failed to create bits");

                    return;
                }

                match user {
                    None => {
                        let twitch_id_s = match twitch_id {
                            Some(id) => id.to_string(),
                            None => "".to_string(),
                        };
                        let new_user = User::builder()
                            .twitch_id(twitch_id_s)
                            .display_name(username.clone())
                            .build();

                        let user_record_id = Some(
                            airtable
                                .create_user(new_user)
                                .await
                                .expect("Failed to create user"),
                        );

                        let bits = models::bits::Bits::builder()
                            .user_id(user_record_id.clone())
                            .display_name(Some(username))
                            .number(number)
                            .message(Some(message))
                            .build();

                        let _ = airtable
                            .create_bits(bits)
                            .await
                            .expect("Failed to create bits");
                    }
                    Some(user) => {
                        let bits = models::bits::Bits::builder()
                            .user_id(Some(user.id.clone()))
                            .display_name(Some(user.fields.display_name))
                            .number(number)
                            .message(Some(message))
                            .build();

                        let _ = airtable
                            .create_bits(bits)
                            .await
                            .expect("Failed to create bits");
                    }
                }
            });
        }
        _ => {}
    }

    (StatusCode::OK, "EventSub".to_string())
}
