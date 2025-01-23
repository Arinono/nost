use super::{user::UserRecordId, RecordId};

pub type BitsRecordId = RecordId;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Bits {
    pub id: usize,
    pub user_id: Option<Vec<UserRecordId>>,
    pub display_name: Option<Vec<String>>,
    pub number: usize,
    pub message: Option<String>,
}

impl Default for Bits {
    fn default() -> Self {
        Self {
            id: 0,
            user_id: None,
            display_name: None,
            number: 0,
            message: None,
        }
    }
}

impl Bits {
    fn new() -> Self {
        Self {
            id: 0,
            user_id: None,
            display_name: None,
            number: 0,
            message: None,
        }
    }

    pub fn builder() -> BitsBuilder {
        BitsBuilder::default()
    }
}

pub struct BitsBuilder {
    bits: Bits,
}

impl Default for BitsBuilder {
    fn default() -> Self {
        Self { bits: Bits::new() }
    }
}

impl BitsBuilder {
    pub fn user_id(mut self, user_id: Option<UserRecordId>) -> Self {
        if let Some(user_id) = user_id {
            self.bits.user_id = Some(vec![user_id]);
        }
        self
    }

    pub fn display_name(mut self, display_name: Option<String>) -> Self {
        if let Some(display_name) = display_name {
            self.bits.display_name = Some(vec![display_name]);
        }
        self
    }

    pub fn number(mut self, number: usize) -> Self {
        self.bits.number = number;
        self
    }

    pub fn message(mut self, message: Option<String>) -> Self {
        if let Some(message) = message {
            self.bits.message = Some(message);
        }
        self
    }

    pub fn build(self) -> Bits {
        self.bits
    }
}
