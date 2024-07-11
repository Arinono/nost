use serenity::all::{CreateEmbed, ExecuteWebhook, Http, Webhook};

use crate::models::misc::SubTier;

pub struct DiscordNotifier {
    http: Http,
    pub webhook: Webhook,
}

impl DiscordNotifier {
    pub async fn new(webhook_url: String) -> Self {
        let http = Http::new("");
        let webhook = Webhook::from_url(&http, &webhook_url)
            .await
            .expect("Invalid webhook URL");

        Self { http, webhook }
    }

    pub async fn new_follower(&self, username: &String) {
        let builder = ExecuteWebhook::new().embed(
            CreateEmbed::default()
                .title("New Follower")
                .field("Username", username, true),
        );

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn new_subscriber(&self, username: &String, tier: &SubTier) {
        let builder = ExecuteWebhook::new().embed(CreateEmbed::default().field(
            "New Subscriber",
            format!(
                "{} has subscribed to the channel with a {} sub!",
                username,
                tier.to_string().to_lowercase()
            ),
            false,
        ));

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn subgift(&self, username: &String, total: usize, tier: &SubTier) {
        let builder = ExecuteWebhook::new().embed(
            CreateEmbed::default()
                .title("New Sub Gift")
                .field("Username", username, true)
                .field("Total", total.to_string(), true)
                .field("Tier", tier.to_string().to_lowercase(), true),
        );

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn bits(&self, username: &String, bits: usize, message: &String) {
        let builder = ExecuteWebhook::new().embed(
            CreateEmbed::default()
                .title("New Bits")
                .field("Username", username, true)
                .field("Bits", bits.to_string(), true)
                .field("Message", message, false),
        );

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }
}
