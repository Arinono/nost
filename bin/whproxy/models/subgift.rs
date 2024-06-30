use super::{misc::SubTier, user::UserRecordId, RecordId};

pub type SubgiftRecordId = RecordId;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Subgift {
    pub id: usize,
    pub user_id: Option<Vec<UserRecordId>>,
    pub display_name: Option<Vec<String>>,
    pub number: usize,
    pub tier: SubTier,
    pub created_at: String,
}

impl Default for Subgift {
    fn default() -> Self {
        Self {
            id: 0,
            user_id: None,
            display_name: None,
            number: 0,
            tier: SubTier::Other,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl Subgift {
    pub fn new(
        user_id: Option<Vec<UserRecordId>>,
        display_name: Option<Vec<String>>,
        number: usize,
        tier: SubTier,
    ) -> Self {
        Self {
            id: 0,
            user_id,
            display_name,
            number,
            tier,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn builder() -> SubgiftBuilder {
        SubgiftBuilder::default()
    }
}

pub struct SubgiftBuilder {
    subgift: Subgift,
}

impl Default for SubgiftBuilder {
    fn default() -> Self {
        Self {
            subgift: Subgift::new(None, None, 0, SubTier::Tier1),
        }
    }
}

impl SubgiftBuilder {
    pub fn user_id(mut self, user_id: Option<UserRecordId>) -> Self {
        if let Some(user_id) = user_id {
            self.subgift.user_id = Some(vec![user_id]);
        }
        self
    }

    pub fn display_name(mut self, display_name: Option<String>) -> Self {
        if let Some(display_name) = display_name {
            self.subgift.display_name = Some(vec![display_name]);
        }
        self
    }

    pub fn number(mut self, number: usize) -> Self {
        self.subgift.number = number;
        self
    }

    pub fn tier(mut self, tier: SubTier) -> Self {
        self.subgift.tier = tier;
        self
    }

    pub fn build(self) -> Subgift {
        self.subgift
    }
}
