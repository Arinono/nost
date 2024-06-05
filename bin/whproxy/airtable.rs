use chrono::{DateTime, Utc};
use std::{sync::Arc, time::Duration};

use retainer::entry::CacheReadGuard;

use crate::env::Secret;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct User {
    id: usize,
    pub display_name: String,
    pub twitch_id: String,
    pub created_at: String,
    pub follower_since: Option<String>,
    pub subscriber_since: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Record {
    id: String,
    #[serde(rename = "createdTime")]
    created_time: DateTime<Utc>,
    pub fields: User,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Records {
    records: Vec<Record>,
}

impl Record {
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
            },
        }
    }
}

pub struct Airtable {
    client: reqwest::Client,
    token: Secret,
    retainer: Arc<retainer::Cache<String, Record>>,
    base_id: String,
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

    pub fn build(self) -> User {
        self.user
    }
}

impl Airtable {
    pub fn new(
        token: Secret,
        base_id: String,
        retainer: Arc<retainer::Cache<String, Record>>,
    ) -> Self {
        let client = reqwest::Client::new();

        Self {
            client,
            token,
            retainer,
            base_id,
        }
    }

    fn base_url(&self) -> String {
        format!("https://api.airtable.com/v0/{}/users", self.base_id)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token.secret_str())
    }

    fn day_in_secs() -> Duration {
        Duration::from_secs(86400)
    }

    pub async fn get_user_by_twitch_id(&self, twitch_id: String) -> Option<Record> {
        if let Some(record) = self.retainer.get(&twitch_id).await {
            return Some(Record::from_cache(record));
        }

        let url = format!(
            "{}?{}&{}",
            self.base_url(),
            "view=abc",
            format!("filterByFormula=(%7Btwitch_id%7D+%3D+'{}')", twitch_id)
        );

        let response = self
            .client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.token.secret_str()),
            )
            .send()
            .await
            .expect("Failed to get user from Airtable");

        if response.status().is_success() {
            let res = response.json().await;
            let records: Records = res.expect("Failed to parse user from Airtable");
            let record = records.records.first();

            match record {
                None => None,
                Some(record) => {
                    self.retainer
                        .insert(twitch_id.clone(), record.clone(), Self::day_in_secs())
                        .await;
                    Some(record.clone())
                }
            }
        } else {
            None
        }
    }

    pub async fn get_most_recent_follow(&self) -> Option<String> {
        let cache_key = "most_recent_follow".to_owned();

        if let Some(record) = self.retainer.get(&cache_key).await {
            return Some(record.fields.display_name.clone());
        }

        let params = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("view", "most_recent_follow")
            .append_pair("maxRecords", "1")
            .finish();

        let url = format!("{}?{}", self.base_url(), params,);

        let response = self
            .client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.token.secret_str()),
            )
            .send()
            .await
            .expect("Failed to get most recent follow from Airtable");

        if response.status().is_success() {
            let record: Records = response
                .json()
                .await
                .expect("Failed to parse most recent follow from Airtable");
            let record = record.records.first();

            match record {
                None => None,
                Some(record) => {
                    self.retainer
                        .insert(
                            "most_recent_follow".to_owned(),
                            record.clone(),
                            Duration::from_secs(300),
                        )
                        .await;

                    Some(record.fields.display_name.clone())
                }
            }
        } else {
            None
        }
    }

    fn fields(user: &User) -> serde_json::Value {
        let mut fields = serde_json::json!({
                "display_name": user.display_name,
                "twitch_id": user.twitch_id,
                "created_at": user.created_at,
        });

        if let Some(followed_at) = &user.follower_since {
            fields["follower_since"] = followed_at.clone().into();
        }

        if let Some(subscribed_at) = &user.subscriber_since {
            fields["subscriber_since"] = subscribed_at.clone().into();
        }

        fields
    }

    pub async fn create_user(&self, user: User) -> Result<(), reqwest::Error> {
        let url = self.base_url();

        let body = serde_json::json!({
            "records": [
                {
                    "fields": Self::fields(&user)
                }
            ]
        });

        let request = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body);

        let response = request.send().await?;

        match response.status().is_success() {
            true => {
                let data = response.json::<Records>().await?;
                let record = data.records.first().unwrap();
                self.retainer
                    .insert(user.twitch_id.clone(), record.clone(), Self::day_in_secs())
                    .await;
                Ok(())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }

    #[allow(dead_code)]
    pub async fn batch_create_users(&self, users: Vec<User>) -> Result<(), reqwest::Error> {
        let url = self.base_url();

        let records = users
            .iter()
            .map(|user| {
                serde_json::json!({
                    "fields": Self::fields(user)
                })
            })
            .collect::<Vec<serde_json::Value>>();

        let body = serde_json::json!({
            "records": records
        });

        let request = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body);

        let response = request.send().await?;

        match response.status().is_success() {
            true => {
                let data = response.json::<Records>().await?;
                for record in data.records {
                    self.retainer
                        .insert(
                            record.fields.twitch_id.clone(),
                            record.clone(),
                            Self::day_in_secs(),
                        )
                        .await;
                }
                Ok(())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }

    pub async fn update_user(&self, record: Record) -> Result<(), reqwest::Error> {
        let url = self.base_url();

        let body = serde_json::json!({
            "records": [
                {
                    "id": record.id,
                    "fields": Self::fields(&record.fields)
                }
            ]
        });

        let request = self
            .client
            .patch(&url)
            .header("Authorization", self.auth_header())
            .json(&body);

        let response = request.send().await?;

        match response.status().is_success() {
            true => {
                let data = response.json::<Records>().await?;
                let record = data.records.first().unwrap();

                self.retainer
                    .update(&record.fields.twitch_id.clone(), |val| {
                        val.fields = record.fields.clone();
                    })
                    .await;

                Ok(())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }
}
