use serde::Deserialize;

use super::{Orm, OrmError};

#[allow(dead_code)]
pub struct Latests;

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LatestFollower {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LatestSubscriber {
    pub name: String,
    pub tier: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LatestSubgift {
    pub name: String,
    pub tier: String,
    pub number: u16,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LatestBit {
    pub name: String,
    pub number: u16,
    pub message: Option<String>,
}

impl Latests {
    #[allow(dead_code)]
    pub async fn get_latest_follower(
        conn: &libsql::Connection,
    ) -> Result<Option<LatestFollower>, OrmError> {
        let query = "select u.display_name name from users u
            inner join latests l on u.id = l.follower
            where u.deleted_at is null
            limit 1
        ";

        let rows = Orm::<LatestFollower>::query(conn, &query.to_string(), vec![]).await?;

        if rows.len() != 1 {
            return Ok(None);
        }

        Ok(Some(rows[0].clone()))
    }

    #[allow(dead_code)]
    pub async fn get_latest_subscriber(
        conn: &libsql::Connection,
    ) -> Result<Option<LatestSubscriber>, OrmError> {
        let query = "select u.display_name name, u.subscription_tier tier from users u
            inner join latests l on u.id = l.subscriber
            where u.deleted_at is null
            limit 1
        ";

        let rows = Orm::<LatestSubscriber>::query(conn, &query.to_string(), vec![]).await?;

        if rows.len() != 1 {
            return Ok(None);
        }

        Ok(Some(rows[0].clone()))
    }

    #[allow(dead_code)]
    pub async fn get_latest_subgift(
        conn: &libsql::Connection,
    ) -> Result<Option<LatestSubgift>, OrmError> {
        let query = "select u.display_name name, s.tier tier, s.number number from users u
            inner join latests l on s.id = l.subgift
            inner join subgifts s on u.id = s.user_id
            where u.deleted_at is null
            limit 1
        ";

        let rows = Orm::<LatestSubgift>::query(conn, &query.to_string(), vec![]).await?;

        if rows.len() != 1 {
            return Ok(None);
        }

        Ok(Some(rows[0].clone()))
    }

    #[allow(dead_code)]
    pub async fn get_latest_bit(conn: &libsql::Connection) -> Result<Option<LatestBit>, OrmError> {
        let query = "select u.display_name name, b.message message, b.number number from users u
            inner join latests l on b.id = l.bit
            inner join bits b on u.id = b.user_id
            where u.deleted_at is null
            limit 1
        ";

        let rows = Orm::<LatestBit>::query(conn, &query.to_string(), vec![]).await?;

        if rows.len() != 1 {
            return Ok(None);
        }

        Ok(Some(rows[0].clone()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{bits::Bit, subgifts::Subgift, user::User, OrmBase};
    use libsql::{Builder, Connection};
    use rand::{distributions::Alphanumeric, Rng};
    use tracing_test::traced_test;

    async fn conn(with_user: bool) -> Connection {
        let rand: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let db_name = format!("test_{}.sqlite", rand.to_lowercase());
        std::fs::copy("tests.sqlite", &db_name).unwrap();

        let db = Builder::new_local(&db_name).build().await.unwrap();
        let conn = db.connect().unwrap();

        if with_user {
            let user = User::from("arinono".to_string(), 42069);
            user.create(&conn).await.unwrap();
        }

        println!("Running test on {}", db_name);

        conn
    }

    #[tokio::test]
    #[traced_test]
    async fn latest_follower() {
        // arrange
        let conn = conn(false).await;
        let mut time = Orm::<()>::now_utc();
        let user_b = User::builder("arinono".to_string(), 42069)
            .follow(time.clone())
            .build();
        let user2_b = User::builder("arinonono".to_string(), 42070)
            .follow(time.clone())
            .build();
        user_b.create(&conn).await.unwrap();
        let id = user2_b.create(&conn).await.unwrap();

        // act
        let latest_follower = Latests::get_latest_follower(&conn).await;

        // assert
        assert!(latest_follower.is_ok());
        let latest_follower = latest_follower.clone().unwrap();
        assert!(latest_follower.is_some());
        assert_eq!(latest_follower.unwrap().name, "arinono".to_string());

        // arrange
        time = Orm::<()>::now_utc();
        let mut user2 = User::get(&conn, id).await.unwrap().unwrap();
        user2.follower_since = Some(time);
        user2.update(&conn).await.unwrap();

        // act
        let latest_follower = Latests::get_latest_follower(&conn).await;

        // assert
        assert!(latest_follower.is_ok());
        let latest_follower = latest_follower.clone().unwrap();
        assert!(latest_follower.is_some());
        assert_eq!(latest_follower.unwrap().name, "arinonono".to_string());
    }

    #[tokio::test]
    #[traced_test]
    async fn latest_subscriber() {
        // arrange
        let conn = conn(false).await;
        let mut time = Orm::<()>::now_utc();
        let user_b = User::builder("arinono".to_string(), 42069)
            .subscribe(time.clone())
            .build();
        let user2_b = User::builder("arinonono".to_string(), 42070)
            .subscribe(time.clone())
            .build();
        user_b.create(&conn).await.unwrap();
        let id = user2_b.create(&conn).await.unwrap();

        // act
        let latest_subscriber = Latests::get_latest_subscriber(&conn).await;

        // assert
        assert!(latest_subscriber.is_ok());
        let latest_subscriber = latest_subscriber.clone().unwrap();

        assert!(latest_subscriber.is_some());
        assert_eq!(
            latest_subscriber.as_ref().unwrap().name,
            "arinono".to_string()
        );
        assert_eq!(latest_subscriber.unwrap().tier, "Tier1".to_string());

        // arrange
        time = Orm::<()>::now_utc();
        let mut user2 = User::get(&conn, id).await.unwrap().unwrap();
        user2.subscriber_since = Some(time);
        user2.subscription_tier = Some("Prime".to_string());
        user2.update(&conn).await.unwrap();

        // act
        let latest_subscriber = Latests::get_latest_subscriber(&conn).await;

        // assert
        assert!(latest_subscriber.is_ok());
        let latest_subscriber = latest_subscriber.clone().unwrap();

        assert!(latest_subscriber.is_some());
        assert_eq!(
            latest_subscriber.as_ref().unwrap().name,
            "arinonono".to_string()
        );
        assert_eq!(latest_subscriber.unwrap().tier, "Prime".to_string());
    }

    #[tokio::test]
    #[traced_test]
    async fn latest_subgift() {
        // arrange
        let conn = conn(false).await;
        let user_b = User::from("arinono".to_string(), 42069);
        let user2_b = User::from("arinonono".to_string(), 42070);
        let id = user_b.create(&conn).await.unwrap();
        let id2 = user2_b.create(&conn).await.unwrap();
        let subgift = Subgift::from(id, 1, "Tier1".to_string());
        subgift.create(&conn).await.unwrap();

        // act
        let latest_subgift = Latests::get_latest_subgift(&conn).await;

        // assert
        assert!(latest_subgift.is_ok());
        let latest_subgift = latest_subgift.clone().unwrap();
        assert!(latest_subgift.is_some());
        assert_eq!(latest_subgift.as_ref().unwrap().name, "arinono".to_string());
        assert_eq!(latest_subgift.as_ref().unwrap().number, 1);
        assert_eq!(latest_subgift.as_ref().unwrap().tier, "Tier1".to_string());

        // arrange
        let subgift = Subgift::from(id2, 4, "Tier2".to_string());
        subgift.create(&conn).await.unwrap();

        // act
        let latest_subgift = Latests::get_latest_subgift(&conn).await;

        // assert
        assert!(latest_subgift.is_ok());
        let latest_subgift = latest_subgift.clone().unwrap();
        assert!(latest_subgift.is_some());
        assert_eq!(
            latest_subgift.as_ref().unwrap().name,
            "arinonono".to_string()
        );
        assert_eq!(latest_subgift.as_ref().unwrap().number, 4);
        assert_eq!(latest_subgift.as_ref().unwrap().tier, "Tier2".to_string());
    }

    #[tokio::test]
    #[traced_test]
    async fn latest_bit() {
        // arrange
        let conn = conn(false).await;
        let user_b = User::from("arinono".to_string(), 42069);
        let user2_b = User::from("arinonono".to_string(), 42070);
        let id = user_b.create(&conn).await.unwrap();
        let id2 = user2_b.create(&conn).await.unwrap();
        let bit = Bit::from(id, 1, None);
        bit.create(&conn).await.unwrap();

        // act
        let latest_bit = Latests::get_latest_bit(&conn).await;

        // assert
        assert!(latest_bit.is_ok());
        let latest_bit = latest_bit.clone().unwrap();
        assert!(latest_bit.is_some());
        assert_eq!(latest_bit.as_ref().unwrap().name, "arinono".to_string());
        assert_eq!(latest_bit.as_ref().unwrap().number, 1);
        assert!(latest_bit.as_ref().unwrap().message.is_none());

        // arrange
        let bit = Bit::from(id2, 4, Some("message".to_string()));
        bit.create(&conn).await.unwrap();

        // act
        let latest_bit = Latests::get_latest_bit(&conn).await;

        // assert
        assert!(latest_bit.is_ok());
        let latest_bit = latest_bit.clone().unwrap();
        assert!(latest_bit.is_some());
        assert_eq!(latest_bit.as_ref().unwrap().name, "arinonono".to_string());
        assert_eq!(latest_bit.as_ref().unwrap().number, 4);
        assert_eq!(
            latest_bit.as_ref().unwrap().message,
            Some("message".to_string())
        );
    }
}
