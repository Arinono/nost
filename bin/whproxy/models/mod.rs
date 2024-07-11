use bits::Bits;
use chrono::{DateTime, Utc};
use retainer::entry::CacheReadGuard;
use subgift::{Subgift, SubgiftRecordId};
use user::{User, UserRecordId};

pub mod bits;
pub mod misc;
pub mod subgift;
pub mod user;

pub type RecordId = String;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UserRecord {
    pub id: UserRecordId,
    #[serde(rename = "createdTime")]
    created_time: DateTime<Utc>,
    pub fields: User,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SubgiftRecord {
    pub id: SubgiftRecordId,
    #[serde(rename = "createdTime")]
    created_time: DateTime<Utc>,
    pub fields: Subgift,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BitsRecord {
    pub id: RecordId,
    #[serde(rename = "createdTime")]
    created_time: DateTime<Utc>,
    pub fields: Bits,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UserRecords {
    pub records: Vec<UserRecord>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SubgiftRecords {
    pub records: Vec<SubgiftRecord>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BitsRecords {
    pub records: Vec<BitsRecord>,
}

impl UserRecord {
    pub fn from_cache(cache_hit: CacheReadGuard<Self>) -> Self {
        Self {
            id: cache_hit.id.clone(),
            created_time: cache_hit.created_time.clone(),
            fields: User {
                id: cache_hit.fields.id,
                display_name: cache_hit.fields.display_name.clone(),
                twitch_id: cache_hit.fields.twitch_id.clone(),
                created_at: cache_hit.fields.created_at.clone(),
                follower_since: cache_hit.fields.follower_since.clone(),
                subscriber_since: cache_hit.fields.subscriber_since.clone(),
                subscription_tier: cache_hit.fields.subscription_tier.clone(),
                subgift_total: cache_hit.fields.subgift_total.clone(),
                subgifts: cache_hit.fields.subgifts.clone(),
                bits: cache_hit.fields.bits.clone(),
            },
        }
    }
}

impl SubgiftRecord {
    pub fn from_cache(cache_hit: CacheReadGuard<Self>) -> Self {
        Self {
            id: cache_hit.id.clone(),
            created_time: cache_hit.created_time.clone(),
            fields: Subgift {
                id: cache_hit.fields.id,
                user_id: cache_hit.fields.user_id.clone(),
                display_name: cache_hit.fields.display_name.clone(),
                number: cache_hit.fields.number.clone(),
                tier: cache_hit.fields.tier.clone(),
                created_at: cache_hit.fields.created_at.clone(),
            },
        }
    }
}

impl BitsRecord {
    pub fn from_cache(cache_hit: CacheReadGuard<Self>) -> Self {
        Self {
            id: cache_hit.id.clone(),
            created_time: cache_hit.created_time.clone(),
            fields: Bits {
                id: cache_hit.fields.id.clone(),
                user_id: cache_hit.fields.user_id.clone(),
                display_name: cache_hit.fields.display_name.clone(),
                number: cache_hit.fields.number.clone(),
                message: cache_hit.fields.message.clone(),
            },
        }
    }
}
