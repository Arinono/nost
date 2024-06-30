use std::{sync::Arc, time::Duration};

use crate::{
    env::Secret,
    models::{
        subgift::{Subgift, SubgiftRecordId},
        user::User,
        SubgiftRecord, SubgiftRecords, UserRecord, UserRecords,
    },
};

pub struct Airtable {
    client: reqwest::Client,
    token: Secret,
    user_cache: Arc<retainer::Cache<String, UserRecord>>,
    subgift_cache: Arc<retainer::Cache<String, SubgiftRecord>>,
    base_id: String,
}

impl Airtable {
    pub fn new(
        token: Secret,
        base_id: String,
        user_cache: Arc<retainer::Cache<String, UserRecord>>,
        subgift_cache: Arc<retainer::Cache<String, SubgiftRecord>>,
    ) -> Self {
        let client = reqwest::Client::new();

        Self {
            client,
            token,
            user_cache,
            subgift_cache,
            base_id,
        }
    }

    fn base_url(&self, table: &str) -> String {
        format!("https://api.airtable.com/v0/{}/{}", self.base_id, table)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token.secret_str())
    }

    fn day_in_secs() -> Duration {
        Duration::from_secs(86400)
    }

    pub async fn get_user_by_twitch_id(&self, twitch_id: String) -> Option<UserRecord> {
        if let Some(record) = self.user_cache.get(&twitch_id).await {
            return Some(UserRecord::from_cache(record));
        }

        let url = format!(
            "{}?{}&{}",
            self.base_url("users"),
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
            let records: UserRecords = res.expect("Failed to parse user from Airtable");
            let record = records.records.first();

            match record {
                None => None,
                Some(record) => {
                    self.user_cache
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

        if let Some(record) = self.user_cache.get(&cache_key).await {
            return Some(record.fields.display_name.clone());
        }

        let params = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("view", "most_recent_follow")
            .append_pair("maxRecords", "1")
            .finish();

        let url = format!("{}?{}", self.base_url("users"), params);

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
            let record: UserRecords = response
                .json()
                .await
                .expect("Failed to parse most recent follow from Airtable");
            let record = record.records.first();

            match record {
                None => None,
                Some(record) => {
                    self.user_cache
                        .insert(
                            "most_recent_follow".to_owned(),
                            record.clone(),
                            Duration::from_secs(30),
                        )
                        .await;

                    Some(record.fields.display_name.clone())
                }
            }
        } else {
            None
        }
    }

    fn user_fields(user: &User) -> serde_json::Value {
        let mut fields = serde_json::json!({
                "display_name": user.display_name,
                "twitch_id": user.twitch_id,
        });

        if let Some(followed_at) = &user.follower_since {
            fields["follower_since"] = followed_at.clone().into();
        }

        if let Some(subscribed_at) = &user.subscriber_since {
            fields["subscriber_since"] = subscribed_at.clone().into();
        }

        fields
    }

    fn subgift_fields(subgift: &Subgift) -> serde_json::Value {
        serde_json::json!({
            "user_id": subgift.user_id,
            "display_name": subgift.display_name,
            "number": subgift.number,
            "tier": subgift.tier,
        })
    }

    pub async fn create_user(&self, user: User) -> Result<(), reqwest::Error> {
        let url = self.base_url("users");

        let body = serde_json::json!({
            "records": [
                {
                    "fields": Self::user_fields(&user)
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
                let data = response.json::<UserRecords>().await?;
                let record = data.records.first().unwrap();
                self.user_cache
                    .insert(user.twitch_id.clone(), record.clone(), Self::day_in_secs())
                    .await;
                Ok(())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }

    #[allow(dead_code)]
    pub async fn batch_create_users(&self, users: Vec<User>) -> Result<(), reqwest::Error> {
        let url = self.base_url("users");

        let records = users
            .iter()
            .map(|user| {
                serde_json::json!({
                    "fields": Self::user_fields(user)
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
                let data = response.json::<UserRecords>().await?;
                for record in data.records {
                    self.user_cache
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

    pub async fn update_user(&self, record: UserRecord) -> Result<(), reqwest::Error> {
        let url = self.base_url("users");

        let body = serde_json::json!({
            "records": [
                {
                    "id": record.id,
                    "fields": Self::user_fields(&record.fields)
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
                let data = response.json::<UserRecords>().await?;
                let record = data.records.first().unwrap();

                self.user_cache
                    .update(&record.fields.twitch_id.clone(), |val| {
                        val.fields = record.fields.clone();
                    })
                    .await;

                Ok(())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }

    pub async fn get_most_recent_subscriber(&self) -> Option<String> {
        let cache_key = "most_recent_subscriber".to_owned();

        if let Some(record) = self.user_cache.get(&cache_key).await {
            return Some(record.fields.display_name.clone());
        }

        let params = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("view", "most_recent_subscriber")
            .append_pair("maxRecords", "1")
            .finish();

        let url = format!("{}?{}", self.base_url("users"), params);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .expect("Failed to get most recent subscriber from Airtable");

        if response.status().is_success() {
            let record: UserRecords = response
                .json()
                .await
                .expect("Failed to parse most recent subscriber from Airtable");
            let record = record.records.first();

            match record {
                None => None,
                Some(record) => {
                    self.user_cache
                        .insert(
                            "most_recent_subscriber".to_owned(),
                            record.clone(),
                            Duration::from_secs(30),
                        )
                        .await;

                    Some(record.fields.display_name.clone())
                }
            }
        } else {
            None
        }
    }

    pub async fn create_subgift(&self, gift: Subgift) -> Result<SubgiftRecordId, reqwest::Error> {
        let url = self.base_url("subgifts");

        let body = serde_json::json!({
            "record": [
                {
                    "fields": Self::subgift_fields(&gift)
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
                let data = response.json::<SubgiftRecords>().await?;
                let record = data.records.first().unwrap();
                let user_id = record
                    .clone()
                    .fields
                    .user_id
                    .expect("User ID is None")
                    .first()
                    .expect("User ID is empty")
                    .clone();

                self.subgift_cache
                    .insert(user_id, record.clone(), Self::day_in_secs())
                    .await;

                Ok(record.id.clone())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }
}
