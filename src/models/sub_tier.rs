use std::fmt::Display;

use twitch_types::SubscriptionTier;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum SubTier {
    #[serde(rename = "Tier1")]
    Tier1,
    #[serde(rename = "Tier2")]
    Tier2,
    #[serde(rename = "Tier3")]
    Tier3,
    #[serde(rename = "Prime")]
    Prime,
    #[serde(rename = "Other")]
    Other,
}

impl Display for SubTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubTier::Tier1 => write!(f, "Tier1"),
            SubTier::Tier2 => write!(f, "Tier2"),
            SubTier::Tier3 => write!(f, "Tier3"),
            SubTier::Prime => write!(f, "Prime"),
            SubTier::Other => write!(f, "Other"),
        }
    }
}

impl From<SubTier> for serde_json::Value {
    fn from(val: SubTier) -> Self {
        serde_json::Value::String(val.to_string())
    }
}

impl SubTier {
    pub fn from(tier: SubscriptionTier) -> Self {
        match tier {
            SubscriptionTier::Tier1 => SubTier::Tier1,
            SubscriptionTier::Tier2 => SubTier::Tier2,
            SubscriptionTier::Tier3 => SubTier::Tier3,
            SubscriptionTier::Prime => SubTier::Prime,
            SubscriptionTier::Other(_) => SubTier::Other,
        }
    }
}
