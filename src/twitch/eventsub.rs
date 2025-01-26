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
    discord::DiscordNotifier,
    models::{self, sub_tier::SubTier},
    AppState,
};
use tables::{Orm, OrmBase, TwitchId};

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

    match event {
        Event::ChannelFollowV2(P {
            message:
                M::Notification(ChannelFollowV2Payload {
                    user_name, user_id, ..
                }),
            ..
        }) => {
            tracing::info!("got follow event from {} ({})", user_name, user_id);

            let database = app_state.database.clone();
            tokio::spawn(async move {
                discord.new_follower(&user_name.to_string()).await;
                let db = database.db().unwrap();
                let conn = database.conn().unwrap();

                let twitch_id: TwitchId = user_id.into();
                let user = tables::user::User::get_by_twitch_id(&conn, twitch_id.0)
                    .await
                    .expect("Failure to retrieve user");

                match user {
                    None => {
                        let new_user =
                            tables::user::User::builder(user_name.to_string(), twitch_id.0)
                                .follow(Orm::<()>::now_utc())
                                .build();

                        new_user.create(&conn).await.expect("Failed to create user");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
                    }
                    Some(mut user) => {
                        user.follower_since = Some(Utc::now().to_rfc3339());
                        user.display_name = user_name.to_string();

                        user.update(&conn).await.expect("Failed to update user");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
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
            let tier = models::sub_tier::SubTier::from(tier);
            tracing::info!(
                "got sub event from {} ({}) tier {}",
                user_name,
                user_id,
                tier,
            );

            let database = app_state.database.clone();
            tokio::spawn(async move {
                discord.new_subscriber(&user_name.to_string(), &tier).await;
                let db = database.db().unwrap();
                let conn = database.conn().unwrap();

                let twitch_id: TwitchId = user_id.into();
                let user = tables::user::User::get_by_twitch_id(&conn, twitch_id.0)
                    .await
                    .expect("Failure to retrieve user");

                match user {
                    None => {
                        let new_user =
                            tables::user::User::builder(user_name.to_string(), twitch_id.0)
                                .subscribe(Orm::<()>::now_utc())
                                .tier(tier.to_string())
                                .build();

                        new_user.create(&conn).await.expect("Failed to create user");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
                    }
                    Some(mut user) => {
                        user.subscriber_since = Some(Orm::<()>::now_utc());
                        user.subscription_tier = Some(tier.to_string());
                        user.display_name = user_name.to_string();

                        user.update(&conn).await.expect("Failed to update user");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
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

            let database = app_state.database.clone();
            tokio::spawn(async move {
                let db = database.db().unwrap();
                let conn = database.conn().unwrap();

                let twitch_id: TwitchId = user_id.into();
                let user = tables::user::User::get_by_twitch_id(&conn, twitch_id.0)
                    .await
                    .expect("Failure to retrieve user");

                match user {
                    None => {
                        tracing::warn!("got sub end event for unknown user");
                        let new_user =
                            tables::user::User::builder(user_name.to_string(), twitch_id.0).build();

                        new_user.create(&conn).await.expect("Failed to create user");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
                    }
                    Some(mut user) => {
                        user.subscriber_since = None;
                        user.subscription_tier = None;
                        user.display_name = user_name.to_string();

                        user.update(&conn).await.expect("Failed to update user");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
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

            let database = app_state.database.clone();
            tokio::spawn(async move {
                let db = database.db().unwrap();
                let conn = database.conn().unwrap();

                let user = match twitch_id.clone() {
                    Some(id) => {
                        let twitch_id: TwitchId = id.into();
                        tables::user::User::get_by_twitch_id(&conn, twitch_id.0)
                            .await
                            .expect("Failure to retrieve user")
                    }
                    None => None,
                };

                if is_anonymous {
                    let subgift =
                        tables::subgifts::Subgift::from_anonymous(total as u16, tier.to_string());

                    subgift
                        .create(&conn)
                        .await
                        .expect("Failed to create subgift");

                    if !app_state.env.dev_mode {
                        db.sync().await.expect("Failed to sync replica");
                    }

                    return;
                }

                match user {
                    None => {
                        let twitch_id: TwitchId = twitch_id
                            .clone()
                            .expect("a twitch_id for non anonymous user")
                            .into();
                        let new_user = tables::user::User::from(username, twitch_id.0);
                        let user_id = new_user.create(&conn).await.expect("Failed to create user");

                        let subgift = tables::subgifts::Subgift::from(
                            user_id,
                            total as u16,
                            tier.to_string(),
                        );

                        subgift
                            .create(&conn)
                            .await
                            .expect("Failed to create subgift");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
                    }
                    Some(user) => {
                        let mut update_user = user.clone();
                        if let Some(user_name) = user_name {
                            update_user.display_name = user_name.to_string();
                        }

                        update_user
                            .update(&conn)
                            .await
                            .expect("Failed to update user");

                        let subgift = tables::subgifts::Subgift::from(
                            user.id,
                            total as u16,
                            tier.to_string(),
                        );

                        subgift
                            .create(&conn)
                            .await
                            .expect("Failed to create subgift");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
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

            let database = app_state.database.clone();
            tokio::spawn(async move {
                let db = database.db().unwrap();
                let conn = database.conn().unwrap();

                let user = match twitch_id.clone() {
                    Some(id) => {
                        let twitch_id: TwitchId = id.into();
                        tables::user::User::get_by_twitch_id(&conn, twitch_id.0)
                            .await
                            .expect("Failure to retrieve user")
                    }
                    None => None,
                };

                if is_anonymous {
                    let bits = tables::bits::Bit::from_anonymous(number as u32, Some(message));

                    bits.create(&conn).await.expect("Failed to create bits");

                    if !app_state.env.dev_mode {
                        db.sync().await.expect("Failed to sync replica");
                    }

                    return;
                }

                match user {
                    None => {
                        let twitch_id: TwitchId = twitch_id
                            .clone()
                            .expect("a twitch_id for non anonymous user")
                            .into();
                        let new_user = tables::user::User::from(username, twitch_id.0);

                        let new_user_id =
                            new_user.create(&conn).await.expect("Failed to create user");

                        let bits =
                            tables::bits::Bit::from(new_user_id, number as u32, Some(message));

                        bits.create(&conn).await.expect("Failed to create bits");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
                    }
                    Some(user) => {
                        let bits = tables::bits::Bit::from(user.id, number as u32, Some(message));

                        bits.create(&conn).await.expect("Failed to create bits");

                        if !app_state.env.dev_mode {
                            db.sync().await.expect("Failed to sync replica");
                        }
                    }
                }
            });
        }
        _ => {}
    }

    (StatusCode::OK, "EventSub".to_string())
}
