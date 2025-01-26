use std::fmt::Debug;
use std::{
    marker::PhantomData,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use libsql::{de, Connection};
use serde::Deserialize;
use tracing::{error, info};

pub mod bits;
pub mod latests;
pub mod subgifts;
pub mod user;

#[derive(Debug, Deserialize)]
pub struct RowId {
    pub id: u64,
}

impl From<String> for RowId {
    fn from(value: String) -> Self {
        RowId {
            id: value.parse::<u64>().expect("Invalid u64"),
        }
    }
}

pub struct TwitchId(pub u64);

impl From<twitch_types::UserId> for TwitchId {
    fn from(value: twitch_types::UserId) -> Self {
        TwitchId(value.to_string().parse::<u64>().expect("Invalid u64"))
    }
}

const SQL_NOW_UTC_ISO: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
#[allow(dead_code)]
const CHRONO_UTC_ISO_FMT: &str = "%Y-%m-%dT%H:%M:%S.%3fZ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrmError {
    NotFound(String, Option<u64>),
    BadInput(String),
    QueryError(String),
    Deserialisation(String),
    NoChange(String),
    Unknown,
}

impl From<serde::de::value::Error> for OrmError {
    fn from(value: serde::de::value::Error) -> Self {
        tracing::error!(kind = "process_error", error = value.to_string());
        OrmError::Deserialisation(value.to_string())
    }
}

impl From<libsql::Error> for OrmError {
    fn from(value: libsql::Error) -> Self {
        use libsql::Error;

        tracing::error!(kind = "query", error = value.to_string());
        match value {
            Error::ColumnNotFound(e) => {
                OrmError::QueryError(format!("Column not found: {}", e).to_string())
            }
            Error::InvalidColumnName(e) => {
                OrmError::QueryError(format!("Column not found: {}", e).to_string())
            }
            Error::InvalidColumnIndex => OrmError::QueryError("Column not found".to_string()),
            Error::SqliteFailure(_, e) => OrmError::QueryError(e),
            // too lazy to do more
            _ => OrmError::Unknown,
        }
    }
}

pub trait OrmBase<T>
where
    T: Sized,
{
    #[allow(dead_code, async_fn_in_trait)]
    async fn create(&self, connection: &Connection) -> Result<u64, OrmError>;
    #[allow(dead_code, async_fn_in_trait)]
    async fn get(connection: &Connection, id: u64) -> Result<Option<T>, OrmError>;
    #[allow(dead_code, async_fn_in_trait)]
    async fn update(&mut self, connection: &Connection) -> Result<(), OrmError>;
    #[allow(dead_code, async_fn_in_trait)]
    async fn delete(&self, connection: &Connection) -> Result<(), OrmError>;
}

pub struct Orm<T> {
    _phantom: PhantomData<T>,
}

impl<T> Orm<T>
where
    T: for<'de> Deserialize<'de> + Debug,
{
    pub fn placeholders(columns: usize) -> String {
        let indices: Vec<usize> = (1..=columns).collect();
        let placeholders: Vec<String> = indices.iter().map(|i| format!("?{}", i)).collect();
        placeholders.join(", ")
    }

    pub fn update_placeholders(columns: &Vec<&str>) -> String {
        let updates = columns
            .iter()
            .enumerate()
            .map(|(idx, col)| format!("{} = ?{}", col, idx + 1))
            .collect::<Vec<String>>();
        updates.join(", ")
    }

    fn now_ts() -> Duration {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
    }

    #[allow(dead_code)]
    pub fn now_utc() -> String {
        let now = SystemTime::now();
        let dt: DateTime<Utc> = now.into();
        format!("{}", dt.format("%+")).to_string()
    }

    pub async fn query(
        conn: &libsql::Connection,
        query: &String,
        replacements: Vec<String>,
    ) -> Result<Vec<T>, OrmError> {
        let qs = Orm::<T>::now_ts();

        let mut rows = match conn.query(query, replacements).await {
            Ok(r) => r,
            Err(e) => {
                let qe = Orm::<T>::now_ts();
                error!(
                    kind = "query",
                    query = query,
                    error = %e,
                    query_duration_ms = (qe - qs).as_millis(),
                    "Query failed"
                );
                return Err(OrmError::from(e));
            }
        };

        let qe = Orm::<T>::now_ts();
        let query_duration = qe - qs;

        let ds = Orm::<T>::now_ts();
        let mut results: Vec<T> = Vec::new();

        while let Ok(Some(row)) = rows.next().await {
            let parsed = de::from_row::<T>(&row);
            match parsed {
                Ok(parsed_row) => results.push(parsed_row),
                Err(e) => {
                    let de = Orm::<T>::now_ts();
                    tracing::error!(
                        kind = "query",
                        query = query,
                        error = %e,
                        query_duration_ms = query_duration.as_millis(),
                        deserialize_duration_ms = (de - ds).as_millis(),
                        "Deserialization failed"
                    );
                    return Err(OrmError::from(e));
                }
            }
        }

        let de = Orm::<T>::now_ts();
        info!(
            kind = "query",
            query = query,
            query_duration_ms = query_duration.as_millis(),
            deserialize_duration_ms = (de - ds).as_millis(),
            "Query completed"
        );

        Ok(results)
    }

    pub async fn execute(
        conn: &libsql::Connection,
        query: &String,
        replacements: Vec<String>,
    ) -> Result<u64, OrmError> {
        let qs = Orm::<T>::now_ts();

        let affected_rows = match conn.execute(query, replacements).await {
            Ok(r) => r,
            Err(e) => {
                let qe = Orm::<T>::now_ts();
                error!(
                    kind = "query",
                    query = query,
                    error = %e,
                    query_duration_ms = (qe - qs).as_millis(),
                    "Query failed"
                );
                return Err(OrmError::from(e));
            }
        };

        let qe = Orm::<T>::now_ts();
        let query_duration = qe - qs;

        info!(
            kind = "query",
            query = query,
            query_duration_ms = query_duration.as_millis(),
            affected_rows = affected_rows,
            "Query completed"
        );

        Ok(affected_rows)
    }
}

#[macro_export]
macro_rules! add_if_present {
    ($columns:expr, $replacements:expr, $instance:expr, $field:ident) => {
        if let Some(value) = &$instance.$field {
            $columns.push(stringify!($field));
            $replacements.push(value.to_string());
        }
    };
}
