use serde::Deserialize;

use crate::{add_if_present, user::User, Orm, OrmBase, OrmError, RowId, SQL_NOW_UTC_ISO};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Bit {
    pub id: u64,
    pub user_id: Option<u64>,
    pub number: u32,
    pub message: Option<String>,
    pub created_at: String,
}

impl Default for Bit {
    fn default() -> Self {
        Self::new()
    }
}

impl Bit {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            id: 0,
            user_id: None,
            number: 0,
            message: None,
            created_at: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn from(user_id: u64, number: u32, message: Option<String>) -> Self {
        Self {
            id: 0,
            user_id: Some(user_id),
            number,
            message,
            created_at: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn from_anonymous(number: u32, message: Option<String>) -> Self {
        Self {
            id: 0,
            user_id: None,
            number,
            message,
            created_at: String::new(),
        }
    }

    #[allow(dead_code)]
    pub async fn create(&self, conn: &libsql::Connection) -> Result<u64, OrmError> {
        if self.number == 0 {
            return Err(OrmError::BadInput("Bits number cannot be 0".to_string()));
        }

        let mut columns = vec!["number"];
        let mut replacements = vec![self.number.to_string()];

        if let Some(id) = self.user_id {
            User::get(conn, id).await?;
            columns.push("user_id");
            replacements.push(id.to_string());
        }

        add_if_present!(columns, replacements, self, message);

        let query = format!(
            "insert into bits (
                {}, created_at
            ) values (
                {}, {}
            ) returning id",
            columns.join(", "),
            Orm::<Bit>::placeholders(columns.len()),
            SQL_NOW_UTC_ISO,
        );

        let rows = Orm::<RowId>::query(conn, &query, replacements).await?;

        match rows.first() {
            None => Err(OrmError::NoChange("No bit created".to_string())),
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
    async fn create_with_errors() {
        // arrange
        let conn = conn(true).await;
        let bit = Bit::from(1, 0, None);

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("Bits number cannot be 0".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_no_user() {
        // arrange
        let conn = conn(false).await;
        let bit = Bit::from(1, 1, None);

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::NoChange("No bit created".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create_0_bits() {
        // arrange
        let conn = conn(true).await;
        let bit = Bit::from(1, 0, None);

        // act
        let res = bit.create(&conn).await;

        // assert
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            OrmError::BadInput("Bits number cannot be 0".to_string())
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn create() {
        // arrange
        let conn = conn(true).await;
        let bit = Bit::from(1, 1, Some("message".to_string()));

        // act
        let res = bit.create(&conn).await;
        dbg!(&res);

        // assert
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 1);

        let mut rows = conn
            .query("select * from bits where id = ?1 limit 1", [1])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let bit_st = de::from_row::<Bit>(&row).unwrap();

        assert_eq!(bit_st.id, 1);
        assert_eq!(bit_st.user_id, Some(1));
        assert_eq!(bit_st.number, 1);
        assert_eq!(bit_st.message, Some("message".to_string()));
        let created_at = NaiveDateTime::parse_from_str(&bit_st.created_at, CHRONO_UTC_ISO_FMT);
        assert!(created_at.is_ok());
    }
}
