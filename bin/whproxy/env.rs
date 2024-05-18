#[derive(Clone)]
pub struct Secret(String);

#[derive(Debug, Clone)]
pub struct Environment {
    pub event_sub_secret: Secret,
    pub twitch_client_id: String,
    pub twitch_client_secret: Secret,
    pub twitch_user_id: String,
    pub twitch_eventsub_callback_url: String,
    pub twitch_user_oauth_callback_url: String,
    pub discord_webhook_url: Secret,
}

impl Secret {
    pub fn secret(&self) -> &[u8] {
        self.0.as_bytes()
    }
    pub fn secret_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret")
            .field("secret", &"********")
            .finish()
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("********")
    }
}

impl std::str::FromStr for Secret {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

trait ToSecret {
    fn to_secret(self) -> Secret;
}

impl ToSecret for String {
    fn to_secret(self) -> Secret {
        Secret(self)
    }
}

impl Environment {
    const PREFIX: &'static str = "NOST_";

    fn string(key: &str) -> String {
        let full_key = format!("{}{}", Self::PREFIX, key);
        std::env::var(&full_key).expect(&format!("{} is required", full_key))
    }

    fn secret(key: &str) -> Secret {
        Self::string(key).to_secret()
    }

    pub fn new() -> Self {
        let _ = dotenvy::dotenv();

        let event_sub_secret = Self::secret("TWITCH_EVENTSUB_SECRET");
        let twitch_client_id = Self::string("TWITCH_CLIENT_ID");
        let twitch_client_secret = Self::secret("TWITCH_CLIENT_SECRET");
        let twitch_user_id = Self::string("TWITCH_USER_ID");
        let twitch_eventsub_callback_url = Self::string("TWITCH_EVENTSUB_CALLBACK_URL");
        let twitch_user_oauth_callback_url = Self::string("TWITCH_USER_OAUTH_CALLBACK_URL");
        let discord_webhook_url = Self::secret("DISCORD_WEBHOOK_URL");

        Self {
            event_sub_secret,
            twitch_client_id,
            twitch_client_secret,
            twitch_user_id,
            twitch_eventsub_callback_url,
            twitch_user_oauth_callback_url,
            discord_webhook_url,
        }
    }
}
