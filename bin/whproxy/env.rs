#[derive(Clone)]
pub struct Secret(String);

#[derive(Debug, Clone)]
pub struct Environment {
    pub event_sub_secret: Secret,
    pub twitch_client_id: String,
    pub twitch_client_secret: Secret,
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

        Self {
            event_sub_secret,
            twitch_client_id,
            twitch_client_secret,
        }
    }
}
