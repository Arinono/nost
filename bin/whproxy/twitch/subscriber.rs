use std::sync::Arc;

use eyre::eyre;
use tokio::sync::RwLock;
use twitch_api::eventsub::channel::ChannelSubscribeV1;
use twitch_api::eventsub::{EventSubSubscription, EventType};
use twitch_api::{eventsub::Transport, HelixClient};
use twitch_oauth2::AppAccessToken;

pub mod subscribe {
    use super::*;

    pub fn subscription_exists<'a>(
        eventsub_callback_url: &'a str,
        broadcaster_id: &'a str,
    ) -> impl 'a + FnMut(&EventSubSubscription) -> bool {
        move |sub: &EventSubSubscription| {
            sub.transport.as_webhook().expect("webhook").callback == eventsub_callback_url
                && sub.version == "1"
                && sub.type_ == EventType::ChannelSubscribe
                && sub
                    .condition
                    .as_object()
                    .expect("channel.follow does not contain an object")
                    .get("broadcaster_user_id")
                    .expect("channel.follow does not contain broadcaster_user_id")
                    .as_str()
                    == Some(&broadcaster_id)
        }
    }

    pub async fn create_subscription<'a>(
        broadcaster_id: &'a str,
        token: &'a Arc<RwLock<AppAccessToken>>,
        helix: &'a HelixClient<'static, reqwest::Client>,
        transport: &'a Transport,
    ) -> Result<(), eyre::Report> {
        tracing::info!("Creating new subscription");
        match helix
            .create_eventsub_subscription(
                ChannelSubscribeV1::broadcaster_user_id(broadcaster_id),
                transport.clone(),
                &*token.read().await,
            )
            .await
        {
            Ok(sub) => {
                tracing::info!("Created subscription: {:#?}", sub);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to create subscription: {:#?}", e);
                Err(eyre!(e))
            }
        }
    }
}

pub mod subscribe_end {
    use twitch_api::eventsub::channel::ChannelSubscriptionEndV1;

    use super::*;

    pub fn subscription_end_exists<'a>(
        eventsub_callback_url: &'a str,
        broadcaster_id: &'a str,
    ) -> impl 'a + FnMut(&EventSubSubscription) -> bool {
        move |sub: &EventSubSubscription| {
            sub.transport.as_webhook().expect("webhook").callback == eventsub_callback_url
                && sub.version == "1"
                && sub.type_ == EventType::ChannelSubscriptionEnd
                && sub
                    .condition
                    .as_object()
                    .expect("channel.follow does not contain an object")
                    .get("broadcaster_user_id")
                    .expect("channel.follow does not contain broadcaster_user_id")
                    .as_str()
                    == Some(&broadcaster_id)
        }
    }

    pub async fn create_subscription<'a>(
        broadcaster_id: &'a str,
        token: &'a Arc<RwLock<AppAccessToken>>,
        helix: &'a HelixClient<'static, reqwest::Client>,
        transport: &'a Transport,
    ) -> Result<(), eyre::Report> {
        tracing::info!("Creating new subscription");
        match helix
            .create_eventsub_subscription(
                ChannelSubscriptionEndV1::broadcaster_user_id(broadcaster_id),
                transport.clone(),
                &*token.read().await,
            )
            .await
        {
            Ok(sub) => {
                tracing::info!("Created subscription: {:#?}", sub);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to create subscription: {:#?}", e);
                Err(eyre!(e))
            }
        }
    }
}
