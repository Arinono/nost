use std::sync::Arc;

use futures::TryStreamExt;
use twitch_api::{
    eventsub::{self as twitch_eventsub, channel::ChannelFollowV2, EventType, Status},
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

        tracing::debug!("Subscriptions: {:#?}", subs);

        let follower_exists = subs.iter().any(|sub| {
            sub.transport.as_webhook().expect("webhook").callback
                == state.env.twitch_eventsub_callback_url
                && sub.version == "2"
                && sub.type_ == EventType::ChannelFollow
                && sub
                    .condition
                    .as_object()
                    .expect("channel.follow does not contain an object")
                    .get("broadcaster_user_id")
                    .expect("channel.follow does not contain broadcaster_user_id")
                    .as_str()
                    == Some(&state.env.twitch_user_id)
        });

        tracing::info!(follower = follower_exists, "existing subs");

        let transport = twitch_eventsub::Transport::webhook(
            state.env.twitch_eventsub_callback_url.clone(),
            state.env.event_sub_secret.secret_str().to_owned(),
        );

        drop(subs);

        if !follower_exists {
            tracing::info!("Creating new subscription");
            let sub = helix
                .create_eventsub_subscription(
                    ChannelFollowV2::new(
                        state.env.twitch_user_id.clone(),
                        state.env.twitch_user_id.clone(),
                    ),
                    transport.clone(),
                    &*token.read().await,
                )
                .await;

            match sub {
                Ok(sub) => tracing::info!("Created subscription: {:#?}", sub),
                Err(e) => {
                    tracing::error!("Failed to create subscription: {:#?}", e);
                    continue;
                }
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
