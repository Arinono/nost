use serenity::all::{Colour, CreateEmbed, ExecuteWebhook, Http, Webhook};

use crate::models::misc::SubTier;

#[derive(Debug, Clone)]
struct EmbedColor(String);

pub struct DiscordNotifier {
    http: Http,
    embed_color: EmbedColor,
    pub webhook: Webhook,
}

impl Into<Colour> for EmbedColor {
    fn into(self) -> Colour {
        Colour::from_rgb(
            u8::from_str_radix(&self.0[1..3], 16).unwrap(),
            u8::from_str_radix(&self.0[3..5], 16).unwrap(),
            u8::from_str_radix(&self.0[5..7], 16).unwrap(),
        )
    }
}

impl DiscordNotifier {
    pub async fn new(webhook_url: String) -> Self {
        let http = Http::new("");
        let embed_color = EmbedColor("0xE5A266".to_string());
        let webhook = Webhook::from_url(&http, &webhook_url)
            .await
            .expect("Invalid webhook URL");

        Self {
            http,
            embed_color,
            webhook,
        }
    }

    pub async fn new_follower(&self, username: &String) {
        let builder = ExecuteWebhook::new().embed(
            CreateEmbed::default()
                .title("New Follower")
                .color(self.embed_color.clone())
                .field("Username", username, true),
        );

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn new_subscriber(&self, username: &String, tier: &SubTier) {
        let builder = ExecuteWebhook::new().embed(
            CreateEmbed::default()
                .color(self.embed_color.clone())
                .field(
                    "New Subscriber",
                    format!(
                        "{} has subscribed to the channel with a {} sub!",
                        username,
                        tier.to_string().to_lowercase()
                    ),
                    false,
                ),
        );

        self.webhook
            .execute(&self.http, false, builder)
            .await
            .expect("Could not execute webhook.");
    }

    pub async fn subgift(&self, username: &String, total: usize, tier: &SubTier) {
        let builder = ExecuteWebhook::new().embed(
            CreateEmbed::default()
                .title("New Sub Gift")
                .color(self.embed_color.clone())
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
                .color(self.embed_color.clone())
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
