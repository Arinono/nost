use super::{bits::BitsRecordId, misc::SubTier, subgift::SubgiftRecordId, RecordId};

pub type UserRecordId = RecordId;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub id: usize,
    pub display_name: String,
    pub twitch_id: String,
    pub created_at: String,
    pub follower_since: Option<String>,
    pub subscriber_since: Option<String>,
    pub subscription_tier: Option<SubTier>,
    pub subgift_total: Option<usize>,
    pub subgifts: Option<Vec<SubgiftRecordId>>,
    pub bits: Option<Vec<BitsRecordId>>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            id: 0,
            display_name: "".to_owned(),
            twitch_id: "".to_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            follower_since: None,
            subscriber_since: None,
            subscription_tier: None,
            subgift_total: None,
            subgifts: None,
            bits: None,
        }
    }
}

impl User {
    pub fn new(name: String, twitch_id: String) -> Self {
        Self {
            id: 0,
            display_name: name,
            twitch_id,
            created_at: chrono::Utc::now().to_rfc3339(),
            follower_since: None,
            subscriber_since: None,
            subscription_tier: None,
            subgift_total: None,
            subgifts: None,
            bits: None,
        }
    }

    pub fn builder() -> UserBuilder {
        UserBuilder::default()
    }
}

pub struct UserBuilder {
    user: User,
}

impl Default for UserBuilder {
    fn default() -> Self {
        Self {
            user: User::new("".to_owned(), "".to_owned()),
        }
    }
}

impl UserBuilder {
    pub fn display_name(mut self, name: String) -> Self {
        self.user.display_name = name;
        self
    }

    pub fn twitch_id(mut self, id: String) -> Self {
        self.user.twitch_id = id;
        self
    }

    pub fn followed_at(mut self, followed_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.user.follower_since = Some(followed_at.to_rfc3339());
        self
    }

    pub fn subscribed_at(mut self, subscribed_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.user.subscriber_since = Some(subscribed_at.to_rfc3339());
        self
    }

    pub fn subscription_tier(mut self, tier: SubTier) -> Self {
        self.user.subscription_tier = Some(tier);
        self
    }

    pub fn subgift_total(mut self, total: usize) -> Self {
        self.user.subgift_total = Some(total);
        self
    }

    pub fn subgifts(mut self, subgifts: Vec<SubgiftRecordId>) -> Self {
        if let Some(cur_subgifts) = &mut self.user.subgifts {
            cur_subgifts.extend(subgifts);
        } else {
            self.user.subgifts = Some(subgifts);
        }
        self
    }

    pub fn bits(mut self, bits: Vec<BitsRecordId>) -> Self {
        if let Some(cur_bits) = &mut self.user.bits {
            cur_bits.extend(bits);
        } else {
            self.user.bits = Some(bits);
        }
        self
    }

    pub fn build(self) -> User {
        self.user
    }
}
