use std::{str::FromStr, sync::Arc, time::Duration};

use crate::{
    env::Secret,
    models::{
        subgift::{Subgift, SubgiftRecordId},
        user::{User, UserRecordId},
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

#[derive(Debug, Clone, serde::Serialize)]
struct CreateRecordFields {
    fields: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
struct UpdateRecordFields {
    id: String,
    fields: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CreateRecords {
    records: Vec<CreateRecordFields>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct UpdateRecords {
    records: Vec<UpdateRecordFields>,
}

#[derive(Debug, Clone)]
pub enum Base {
    Users,
    Subgifts,
}

impl FromStr for Base {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "users" => Ok(Self::Users),
            "subgifts" => Ok(Self::Subgifts),
            _ => Err(format!("Invalid base: {}", s)),
        }
    }
}

impl From<Base> for &'static str {
    fn from(base: Base) -> &'static str {
        match base {
            Base::Users => "users",
            Base::Subgifts => "subgifts",
        }
    }
}

impl CreateRecords {
    fn from(fields: CreateRecordFields) -> Self {
        Self {
            records: vec![fields],
        }
    }

    fn from_multi(fields: Vec<CreateRecordFields>) -> Self {
        Self { records: fields }
    }
}

impl Into<serde_json::Value> for CreateRecordFields {
    fn into(self) -> serde_json::Value {
        serde_json::json!({
            "fields": self.fields,
        })
    }
}

impl UpdateRecords {
    fn from(fields: UpdateRecordFields) -> Self {
        Self {
            records: vec![fields],
        }
    }
}

impl Into<serde_json::Value> for UpdateRecordFields {
    fn into(self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "fields": self.fields,
        })
    }
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

    pub async fn get_user_by_record_id(&self, user_id: UserRecordId) -> Option<UserRecord> {
        if let Some(record) = self.user_cache.get(&user_id).await {
            return Some(UserRecord::from_cache(record));
        }

        let url = format!("{}/{}", self.base_url("users"), user_id);

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
            let record: UserRecord = res.expect("Failed to parse user from Airtable");

            self.user_cache
                .insert(record.id.clone(), record.clone(), Self::day_in_secs())
                .await;
            Some(record.clone())
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
                        .insert(cache_key.clone(), record.clone(), Duration::from_secs(30))
                        .await;

                    Some(record.fields.display_name.clone())
                }
            }
        } else {
            None
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
                        .insert(cache_key.clone(), record.clone(), Duration::from_secs(30))
                        .await;

                    Some(record.fields.display_name.clone())
                }
            }
        } else {
            None
        }
    }

    pub async fn get_most_recent_subgift(&self) -> Option<String> {
        let cache_key = "most_recent_subgift".to_owned();

        if let Some(record) = self.subgift_cache.get(&cache_key).await {
            let display_name = record.fields.display_name.clone();
            let true_name = match &display_name {
                Some(name) => name.first().expect("Display name is empty").clone(),
                None => "Anonymous".to_owned(),
            };
            return Some(format!("{} ({})", true_name, record.fields.number));
        }

        let params = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("view", "most_recent_subgift")
            .append_pair("maxRecords", "1")
            .finish();

        let url = format!("{}?{}", self.base_url("subgifts"), params);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .expect("Failed to get most recent subscriber from Airtable");

        if response.status().is_success() {
            let record: SubgiftRecords = response
                .json()
                .await
                .expect("Failed to parse most recent subscriber from Airtable");
            let record = record.records.first();

            match record {
                None => None,
                Some(record) => {
                    let user_id = match &record.fields.user_id {
                        None => None,
                        Some(user_id) => Some(user_id.first().expect("User ID is empty").clone()),
                    };

                    match &user_id {
                        None => Some(format!("{} ({})", "Anonymous", record.fields.number)),
                        Some(user_id) => {
                            let mut record = record.clone();
                            let user = self
                                .get_user_by_record_id(user_id.clone())
                                .await
                                .expect("User not found");

                            record.fields.display_name =
                                Some(vec![user.fields.display_name.clone()]);
                            self.subgift_cache
                                .insert(cache_key.clone(), record.clone(), Duration::from_secs(30))
                                .await;

                            Some(format!(
                                "{} ({})",
                                user.fields.display_name, record.fields.number
                            ))
                        }
                    }
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

        match &user.follower_since {
            Some(follower_since) => {
                fields["follower_since"] = follower_since.clone().into();
            }
            None => {
                fields["follower_since"] = serde_json::Value::Null;
            }
        }

        match &user.subscriber_since {
            Some(subscriber_since) => {
                fields["subscriber_since"] = subscriber_since.clone().into();
            }
            None => {
                fields["subscriber_since"] = serde_json::Value::Null;
            }
        }

        match &user.subscription_tier {
            Some(subscription_tier) => {
                fields["subscription_tier"] = subscription_tier.clone().into();
            }
            None => {
                fields["subscription_tier"] = serde_json::Value::Null;
            }
        }

        match &user.subgift_total {
            Some(subgift_total) => {
                fields["subgift_total"] = subgift_total.clone().into();
            }
            None => {
                fields["subgift_total"] = serde_json::Value::Null;
            }
        }

        fields
    }

    fn subgift_fields(subgift: &Subgift) -> serde_json::Value {
        let mut fields = serde_json::json!({
            "number": subgift.number,
            "tier": subgift.tier,
        });

        match &subgift.user_id {
            Some(user_id) => {
                fields["user_id"] = user_id.clone().into();
            }
            None => {
                fields["user_id"] = serde_json::Value::Array(vec![]);
            }
        }

        fields
    }

    pub async fn create_user(&self, user: User) -> Result<UserRecordId, reqwest::Error> {
        let url = self.base_url("users");

        let body = CreateRecords::from(CreateRecordFields {
            fields: Self::user_fields(&user),
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
                Ok(record.id.clone())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }

    #[allow(dead_code)]
    pub async fn batch_create_users(&self, users: Vec<User>) -> Result<(), reqwest::Error> {
        let url = self.base_url("users");

        let records = users
            .iter()
            .map(|user| CreateRecordFields {
                fields: Self::user_fields(user),
            })
            .collect::<Vec<CreateRecordFields>>();

        let body = CreateRecords::from_multi(records);
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

        let mut body = serde_json::json!({
            "performUpsert": {
                "fieldsToMergeOn": ["twitch_id"]
            },
        });
        body["records"] = UpdateRecords::from(UpdateRecordFields {
            id: record.id.clone(),
            fields: Self::user_fields(&record.fields),
        })
        .records
        .into();

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

    pub async fn create_subgift(&self, gift: Subgift) -> Result<SubgiftRecordId, reqwest::Error> {
        let url = self.base_url("subgifts");

        let body = CreateRecords::from(CreateRecordFields {
            fields: Self::subgift_fields(&gift),
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
                match &record.fields.user_id {
                    Some(user_id) => {
                        let user_id = user_id.first().expect("User ID is empty");
                        self.subgift_cache
                            .insert(user_id.clone(), record.clone(), Self::day_in_secs())
                            .await;
                    }
                    None => {
                        tracing::warn!("User ID is None");
                    }
                }
                Ok(record.id.clone())
            }
            false => Err(response.error_for_status().unwrap_err()),
        }
    }
}
