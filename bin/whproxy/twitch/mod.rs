pub mod eventsub;
mod follower;
pub mod oauth;
mod subgift;
mod subscriber;

use std::sync::Arc;

use futures::TryStreamExt;
use twitch_api::{
    eventsub::{self as twitch_eventsub, Status},
    HelixClient,
};

use crate::AppState;

pub async fn eventsub_register(
    state: AppState,
    helix: HelixClient<'static, reqwest::Client>,
    token: Arc<tokio::sync::RwLock<twitch_oauth2::AppAccessToken>>,
) -> eyre::Result<()> {
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // check every day
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 60 * 60));

    loop {
        interval.tick().await;

        tracing::info!("Checking EventSub subscriptions");
        let subs = helix
            .get_eventsub_subscriptions(Status::Enabled, None, None, &*token.read().await)
            .map_ok(|events| {
                futures::stream::iter(events.subscriptions.into_iter().map(Ok::<_, eyre::Report>))
            })
            .try_flatten()
            // filter out websockets
            .try_filter(|event| futures::future::ready(event.transport.is_webhook()))
            .try_collect::<Vec<_>>()
            .await?;

        tracing::info!("Subscriptions: {:#?}", subs);

        let follower_exists = subs.iter().any(follower::subscription_exists(
            &state.env.twitch_eventsub_callback_url,
            &state.env.twitch_broadcaster_id,
        ));

        let subscribe_exists = subs.iter().any(subscriber::subscribe::subscription_exists(
            &state.env.twitch_eventsub_callback_url,
            &state.env.twitch_broadcaster_id,
        ));

        let subscribe_end_exists =
            subs.iter()
                .any(subscriber::subscribe_end::subscription_end_exists(
                    &state.env.twitch_eventsub_callback_url,
                    &state.env.twitch_broadcaster_id,
                ));

        let subgift_exists = subs.iter().any(subgift::subgift_exists(
            &state.env.twitch_eventsub_callback_url,
            &state.env.twitch_broadcaster_id,
        ));

        tracing::info!(
            follower = follower_exists,
            subscriber = subscribe_exists,
            subscriber_end = subscribe_end_exists,
            subgift = subgift_exists,
            "existing subs"
        );

        let transport = twitch_eventsub::Transport::webhook(
            state.env.twitch_eventsub_callback_url.clone(),
            state.env.event_sub_secret.secret_str().to_owned(),
        );

        drop(subs);

        if !follower_exists {
            if follower::create_subscription(
                &state.env.twitch_broadcaster_id,
                &state.env.twitch_moderator_id,
                &token,
                &helix,
                &transport,
            )
            .await
            .is_err()
            {
                continue;
            }
        }

        // can't register these events locally
        if !state.env.dev_mode {
            if !subscribe_exists {
                if subscriber::subscribe::create_subscription(
                    &state.env.twitch_broadcaster_id,
                    &token,
                    &helix,
                    &transport,
                )
                .await
                .is_err()
                {
                    continue;
                }
            }

            if !subscribe_end_exists {
                if subscriber::subscribe_end::create_subscription(
                    &state.env.twitch_broadcaster_id,
                    &token,
                    &helix,
                    &transport,
                )
                .await
                .is_err()
                {
                    continue;
                }
            }

            if !subgift_exists {
                if subgift::create_subscription(
                    &state.env.twitch_broadcaster_id,
                    &token,
                    &helix,
                    &transport,
                )
                .await
                .is_err()
                {
                    continue;
                }
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
