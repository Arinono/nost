use serenity::all::{ExecuteWebhook, Http, Webhook};

use crate::models::misc::SubTier;

pub struct DiscordNotifier {
    http: Http,
    username: String,
    pub webhook: Webhook,
}

impl DiscordNotifier {
    pub async fn new(webhook_url: String) -> Self {
        let http = Http::new("");
        let webhook = Webhook::from_url(&http, &webhook_url)
            .await
            .expect("Invalid webhook URL");

        Self {
            http,
            webhook,
            username: "nost".to_owned(),
        }
    }

    pub async fn new_follower(&self, username: &String) {
        let builder = ExecuteWebhook::new()
            .content(format!("{} has followed the channel!", username))
            .username(&self.username);

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn new_subscriber(&self, username: &String, tier: &SubTier) {
        let builder = ExecuteWebhook::new()
            .content(format!(
                "{} has subscribed to the channel with a {} sub!",
                username,
                tier.to_string().to_lowercase()
            ))
            .username(&self.username);

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn subgift(&self, username: &String, total: usize, tier: &SubTier) {
        let builder = ExecuteWebhook::new()
            .content(format!(
                "{} has gifted {} {} subscriptions!",
                username,
                total,
                tier.to_string().to_lowercase()
            ))
            .username(&self.username);

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }
}
