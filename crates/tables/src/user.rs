use serde::Deserialize;

use crate::{add_if_present, Orm, SQL_NOW_UTC_ISO};

use super::{OrmBase, OrmError, RowId};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub display_name: String,
    pub twitch_id: u64,
    pub follower_since: Option<String>,
    pub subscriber_since: Option<String>,
    pub subgift_total: Option<usize>,
    pub subscription_tier: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserBuilder(pub User);

impl UserBuilder {
    #[allow(dead_code)]
    pub fn follow(mut self, date: String) -> Self {
        self.0.follower_since = Some(date);
        self
    }

    #[allow(dead_code)]
    pub fn subscribe(mut self, date: String) -> Self {
        self.0.subscriber_since = Some(date);
        self
    }

    #[allow(dead_code)]
    pub fn tier(mut self, tier: String) -> Self {
        self.0.subscription_tier = Some(tier);
        self
    }

    #[allow(dead_code)]
    pub fn created_at(mut self, created_at: String) -> Self {
        self.0.created_at = created_at;
        self
    }

    #[allow(dead_code)]
    pub fn build(mut self) -> User {
        if self.0.subscriber_since.is_some() && self.0.subscription_tier.is_none() {
            self.0.subscription_tier = Some("Tier1".to_string());
        }
        if self.0.subscription_tier.is_some() && self.0.subscriber_since.is_none() {
            self.0.subscriber_since = Some(Orm::<()>::now_utc());
        }
        self.0.clone()
    }
}

impl Default for User {
    fn default() -> Self {
        Self::new()
    }
}

impl User {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            id: 0,
            display_name: String::new(),
            twitch_id: 0,
            follower_since: None,
            subscriber_since: None,
            subscription_tier: None,
            subgift_total: None,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        }
    }

    #[allow(dead_code)]
    pub fn from(display_name: String, twitch_id: u64) -> Self {
        Self {
            id: 0,
            display_name,
            twitch_id,
            follower_since: None,
            subscriber_since: None,
            subscription_tier: None,
            subgift_total: None,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        }
    }

    #[allow(dead_code)]
    pub fn builder(display_name: String, twitch_id: u64) -> UserBuilder {
        UserBuilder(User::from(display_name.clone(), twitch_id))
    }

    fn validate_tier(tier: String) -> Result<(), OrmError> {
        match tier.as_str() {
            "Tier1" | "Tier2" | "Tier3" | "Prime" | "Other" => Ok(()),
            _ => Err(OrmError::BadInput("Invalid sub tier name".to_string())),
        }
    }

    fn validate(&self) -> Result<(), OrmError> {
        if self.subscription_tier.is_some() {
            User::validate_tier(self.subscription_tier.clone().unwrap())?;
        }

        if self.subscriber_since.is_some() && self.subscription_tier.is_none() {
            return Err(OrmError::BadInput(
                "subscriber_since requires subscriber_tier to be set".to_string(),
            ));
        }

        if self.subscription_tier.is_some() && self.subscriber_since.is_none() {
            return Err(OrmError::BadInput(
                "subscriber_tier requires subscriber_since to be set".to_string(),
            ));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_by_twitch_id(
        conn: &libsql::Connection,
        id: u64,
    ) -> Result<Option<Self>, OrmError> {
        let query = "select * from users
            where twitch_id = ?1
                and deleted_at is null
            limit 1
        ";
        let replacements = vec![id.to_string()];

        let rows = Orm::<User>::query(conn, &query.to_string(), replacements).await?;

        if rows.len() != 1 {
            return Ok(None);
        }

        Ok(Some(rows[0].clone()))
    }
}

impl OrmBase<User> for User {
    async fn create(&self, conn: &libsql::Connection) -> Result<u64, OrmError> {
        self.validate()?;

        let mut columns = vec!["display_name", "twitch_id"];
        let mut replacements = vec![self.display_name.clone(), self.twitch_id.to_string()];

        add_if_present!(columns, replacements, self, follower_since);
        add_if_present!(columns, replacements, self, subscriber_since);
        add_if_present!(columns, replacements, self, subgift_total);
        add_if_present!(columns, replacements, self, subscription_tier);
        add_if_present!(columns, replacements, self, follower_since);

        let query = format!(
            "insert into users (
                {}, created_at, updated_at
            ) values (
                {}, {}, {}
            ) returning id",
            columns.join(", "),
            Orm::<User>::placeholders(columns.len()),
            SQL_NOW_UTC_ISO,
            SQL_NOW_UTC_ISO
        );

        let rows = Orm::<RowId>::query(conn, &query, replacements).await?;

        match rows.first() {
            None => Err(OrmError::NoChange("No user created".to_string())),
            Some(row) => Ok(row.id),
        }
    }

    async fn get(conn: &libsql::Connection, id: u64) -> Result<Option<Self>, OrmError> {
        let query = "select * from users
            where id = ?1
                and deleted_at is null
            limit 1
            ";
        let replacements: Vec<String> = vec![id.to_string()];

        let rows = Orm::<User>::query(conn, &query.to_string(), replacements).await?;

        if rows.len() != 1 {
            return Ok(None);
        }

        Ok(Some(rows[0].clone()))
    }

    async fn update(&mut self, conn: &libsql::Connection) -> Result<(), OrmError> {
        if User::get(conn, self.id).await?.is_none() {
            return Err(OrmError::NotFound("update user".to_string(), Some(self.id)));
        }

        self.validate()?;

        let mut columns = vec!["display_name"];
        let mut replacements = vec![self.display_name.clone()];

        add_if_present!(columns, replacements, self, follower_since);
        add_if_present!(columns, replacements, self, subscriber_since);
        add_if_present!(columns, replacements, self, subgift_total);
        add_if_present!(columns, replacements, self, subscription_tier);
        add_if_present!(columns, replacements, self, follower_since);

        let query = format!(
            "update users set
                updated_at = {},
                {}
            where id = ?{} and deleted_at is null",
            SQL_NOW_UTC_ISO,
            Orm::<User>::update_placeholders(&columns),
            columns.len() + 1,
        );
        replacements.push(self.id.to_string());

        Orm::<User>::execute(conn, &query, replacements).await?;

        let user_st = User::get(conn, self.id).await?.unwrap();
        self.id = user_st.id;
        self.twitch_id = user_st.twitch_id;
        self.display_name = user_st.display_name.clone();
        self.follower_since = user_st.follower_since;
        self.subscriber_since = user_st.subscriber_since;
        self.subscription_tier = user_st.subscription_tier;
        self.subgift_total = user_st.subgift_total;
        self.created_at = user_st.created_at;
        self.updated_at = user_st.updated_at;
        self.deleted_at = user_st.deleted_at;

        Ok(())
    }

    async fn delete(&self, conn: &libsql::Connection) -> Result<(), OrmError> {
        if User::get(conn, self.id).await?.is_none() {
            return Err(OrmError::NotFound("delete user".to_string(), Some(self.id)));
        };

        let query = format!(
            "update users
                set deleted_at = {}
            where id = ?1 and deleted_at is null",
            SQL_NOW_UTC_ISO,
        );
        let replacements: Vec<String> = vec![self.id.to_string()];

        Orm::<User>::execute(conn, &query.to_string(), replacements).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::CHRONO_UTC_ISO_FMT;

    use super::*;
    use chrono::{NaiveDateTime, Utc};
    use libsql::{de, Builder, Connection};
    use rand::{distributions::Alphanumeric, Rng};
    use tracing_test::traced_test;

    async fn conn() -> Connection {
        let rand: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let db_name = format!("test_{}.sqlite", rand.to_lowercase());
        std::fs::copy("tests.sqlite", &db_name).unwrap();

        let db = Builder::new_local(&db_name).build().await.unwrap();
        let conn = db.connect().unwrap();

        println!("Running test on {}", db_name);

        conn
    }

    #[tokio::test]
    #[traced_test]
    async fn create_empty() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);

        // act
        let res = user.create(&conn).await;

        // assert
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 1);
        let mut rows = conn
            .query("select * from users where id = ?1 limit 1", [1])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let user_st = de::from_row::<User>(&row).unwrap();

        assert_eq!(user_st.id, 1);
        assert_eq!(user_st.display_name, "arinono".to_string());
        assert_eq!(user_st.twitch_id, 42069);
        assert_eq!(user_st.follower_since, None);
        assert_eq!(user_st.subscription_tier, None);
        assert_eq!(user_st.subscriber_since, None);
        assert_eq!(user_st.subgift_total, None);

        let created_at = NaiveDateTime::parse_from_str(&user_st.created_at, CHRONO_UTC_ISO_FMT);
        assert!(created_at.is_ok());
        let updated_at = NaiveDateTime::parse_from_str(&user_st.updated_at, CHRONO_UTC_ISO_FMT);
        assert!(updated_at.is_ok());
        let now = Utc::now();
        assert_eq!(
            now.timestamp() - created_at.unwrap().and_utc().timestamp(),
            0
        );
        assert_eq!(
            now.timestamp() - updated_at.unwrap().and_utc().timestamp(),
            0
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_with_info() {
        // arrange
        let conn = conn().await;
        let mut user = User::from("arinono".to_string(), 42069);
        user.follower_since = Some(Utc::now().to_string());
        user.subscriber_since = Some(Utc::now().to_string());
        user.subgift_total = Some(123);
        user.subscription_tier = Some("Tier1".to_string());

        // act
        let res = user.create(&conn).await;

        // assert
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 1);
        let mut rows = conn
            .query("select * from users where id = ?1 limit 1", [1])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let user_st = de::from_row::<User>(&row).unwrap();

        assert!(user_st.follower_since.is_some());
        assert_eq!(user_st.subscription_tier, Some("Tier1".to_string()));
        assert!(user_st.subscriber_since.is_some());
        assert_eq!(user_st.subgift_total, Some(123));
    }

    #[tokio::test]
    #[traced_test]
    async fn create_with_errors_subscription_since() {
        // arrange
        let conn = conn().await;
        let mut user = User::from("arinono".to_string(), 42069);

        // act
        user.subscriber_since = Some(Utc::now().to_string());
        user.subscription_tier = None;
        let res = user.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("subscriber_since requires subscriber_tier to be set".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_with_errors_subscriber_tier() {
        // arrange
        let conn = conn().await;
        let mut user = User::from("arinono".to_string(), 42069);

        // act
        user.subscriber_since = None;
        user.subscription_tier = Some("Tier1".to_string());
        let res = user.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("subscriber_tier requires subscriber_since to be set".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_with_errors_subscriber_tier_format() {
        // arrange
        let conn = conn().await;
        let mut user = User::from("arinono".to_string(), 42069);

        // act
        user.subscriber_since = Some(Utc::now().to_string());
        user.subscription_tier = Some("Invalid".to_string());
        let res = user.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("Invalid sub tier name".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_with_errors_twitch_id() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let user_2 = User::from("arinono".to_string(), 42069);

        // act
        let _ = user.create(&conn).await;
        let res = user_2.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::NoChange("No user created".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn get_found() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();

        // act
        let user_st = User::get(&conn, id).await;

        // assert
        assert!(user_st.is_ok());
        let user_st = user_st.clone().unwrap();
        assert!(user_st.is_some());
        assert_eq!(user_st.as_ref().unwrap().display_name, "arinono");
        assert_eq!(user_st.unwrap().twitch_id, 42069);
    }

    #[tokio::test]
    #[traced_test]
    async fn get_not_found() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();

        // act
        let user_st = User::get(&conn, id + 1).await;

        // assert
        assert!(user_st.is_ok());
        let user_st = user_st.clone().unwrap();
        assert!(user_st.is_none());
    }

    #[tokio::test]
    #[traced_test]
    async fn delete_found() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();
        let user_st = User::get(&conn, id).await.unwrap();

        // act
        let user_st = user_st.unwrap().delete(&conn).await;

        // assert
        assert!(user_st.is_ok());
    }

    #[tokio::test]
    #[traced_test]
    async fn update() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();
        let mut user_st = User::get(&conn, id).await.unwrap().unwrap();
        let created_at = user_st.created_at.clone();
        let updated_at = user_st.updated_at.clone();

        // act
        user_st.display_name = "changed".to_string();
        let now_utc_str = Utc::now().to_utc().to_string();
        user_st.follower_since = Some(now_utc_str.clone());
        let res = user_st.update(&conn).await;

        // assert
        assert!(res.is_ok());
        assert_eq!(user_st.display_name, "changed".to_string());
        assert_eq!(created_at, user_st.created_at);
        assert_eq!(user.twitch_id, user_st.twitch_id);
        assert_ne!(updated_at, user_st.updated_at);
        assert_eq!(user_st.follower_since, Some(now_utc_str));
    }

    #[tokio::test]
    #[traced_test]
    async fn update_not_found() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();

        // act
        let mut user_2 = User::from("arinono".to_string(), 42070);
        user_2.id = id + 1;
        user_2.display_name = "changed".to_string();
        let res = user_2.update(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::NotFound("update user".to_string(), Some(id + 1))
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn update_with_errors_subscriber_since() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();
        let mut user_st = User::get(&conn, id).await.unwrap().unwrap();

        // act
        user_st.subscription_tier = None;
        user_st.subscriber_since = Some(String::new());
        let res = user_st.update(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("subscriber_since requires subscriber_tier to be set".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn update_with_errors_subscription_tier() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let id = user.create(&conn).await.unwrap();
        let mut user_st = User::get(&conn, id).await.unwrap().unwrap();

        // act
        user_st.subscriber_since = None;
        user_st.subscription_tier = Some("Tier1".to_string());
        let res = user_st.update(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("subscriber_tier requires subscriber_since to be set".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn get_by_twitch_id() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let _ = user.create(&conn).await.unwrap();

        // act
        let user_st = User::get_by_twitch_id(&conn, 42069).await;

        // assert
        assert!(user_st.is_ok());
        let user_st = user_st.clone().unwrap();
        assert!(user_st.is_some());
        assert_eq!(user_st.as_ref().unwrap().display_name, "arinono");
        assert_eq!(user_st.unwrap().twitch_id, 42069);
    }

    #[tokio::test]
    #[traced_test]
    async fn get_by_twitch_id_not_found() {
        // arrange
        let conn = conn().await;
        let user = User::from("arinono".to_string(), 42069);
        let _ = user.create(&conn).await.unwrap();

        // act
        let user_st = User::get_by_twitch_id(&conn, 42070).await;

        // assert
        assert!(user_st.is_ok());
        let user_st = user_st.clone().unwrap();
        assert!(user_st.is_none());
    }
}
