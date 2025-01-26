use serde::Deserialize;

use super::{user::User, Orm, OrmBase, OrmError, RowId, SQL_NOW_UTC_ISO};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Subgift {
    pub id: u64,
    pub user_id: Option<u64>,
    pub number: u16,
    pub tier: String,
    pub created_at: String,
}

impl Default for Subgift {
    fn default() -> Self {
        Self::new()
    }
}

impl Subgift {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            id: 0,
            user_id: None,
            number: 0,
            tier: String::new(),
            created_at: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn from(user_id: u64, number: u16, tier: String) -> Self {
        Self {
            id: 0,
            user_id: Some(user_id),
            number,
            tier,
            created_at: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn from_anonymous(number: u16, tier: String) -> Self {
        Self {
            id: 0,
            user_id: None,
            number,
            tier,
            created_at: String::new(),
        }
    }

    #[allow(dead_code)]
    fn validate(&self) -> Result<(), OrmError> {
        if self.number == 0 {
            return Err(OrmError::BadInput(
                "Subgifts number cannot be 0".to_string(),
            ));
        }

        match self.tier.as_str() {
            "Tier1" | "Tier2" | "Tier3" | "Prime" | "Other" => Ok(()),
            _ => Err(OrmError::BadInput("Invalid sub tier name".to_string())),
        }
    }

    #[allow(dead_code)]
    pub async fn create(&self, conn: &libsql::Connection) -> Result<u64, OrmError> {
        self.validate()?;

        let mut columns: Vec<String> = vec!["number".to_string(), "tier".to_string()];
        let mut replacements = vec![self.number.to_string(), self.tier.clone()];

        if let Some(id) = self.user_id {
            User::get(conn, id).await?;
            columns.push("user_id".to_string());
            replacements.push(id.to_string());
        }

        let query = format!(
            "insert into subgifts (
                {}, created_at
            ) values (
                {}, {}
            ) returning id",
            columns.join(", "),
            Orm::<Subgift>::placeholders(columns.len()),
            SQL_NOW_UTC_ISO
        );

        let rows = Orm::<RowId>::query(conn, &query, replacements).await?;

        match rows.first() {
            None => Err(OrmError::NoChange("No subgift created".to_string())),
            Some(row) => Ok(row.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{user::User, OrmBase, CHRONO_UTC_ISO_FMT};

    use super::*;
    use chrono::NaiveDateTime;
    use libsql::{de, Builder, Connection};
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
    async fn create_no_user() {
        // arrange
        let conn = conn(false).await;
        let bit = Subgift::from(1, 1, "Tier1".to_string());

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::NoChange("No subgift created".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_number_0() {
        // arrange
        let conn = conn(true).await;
        let bit = Subgift::from(1, 0, String::new());

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("Subgifts number cannot be 0".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_number_tier() {
        // arrange
        let conn = conn(true).await;
        let bit = Subgift::from(1, 1, "Invalid".to_string());

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("Invalid sub tier name".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create() {
        // arrange
        let conn = conn(true).await;
        let bit = Subgift::from(1, 1, "Tier1".to_string());

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 1);

        let mut rows = conn
            .query("select * from subgifts where id = ?1 limit 1", [1])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let subgift_st = de::from_row::<Subgift>(&row).unwrap();

        assert_eq!(subgift_st.id, 1);
        assert_eq!(subgift_st.user_id, Some(1));
        assert_eq!(subgift_st.number, 1);
        let created_at = NaiveDateTime::parse_from_str(&subgift_st.created_at, CHRONO_UTC_ISO_FMT);
        assert!(created_at.is_ok());
    }
}
