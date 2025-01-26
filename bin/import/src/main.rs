use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Amsterdam;
use indicatif::ProgressBar;
use serde::Deserialize;
use std::{collections::HashMap, error::Error};
use tables::{bits, subgifts, user, Orm, OrmBase};

#[derive(Debug, Clone, Deserialize)]
struct User {
    pub id: u64,
    pub display_name: String,
    pub twitch_id: u64,
    pub created_at: String,
    pub follower_since: Option<String>,
    pub subscriber_since: Option<String>,
    // pub subgift_total: Option<u64>,
    pub subscription_tier: Option<String>,
}

type Users = Vec<User>;

#[derive(Debug, Clone, Deserialize)]
struct Bit {
    // pub id: u16,
    pub user_id: Option<u64>,
    pub message: Option<String>,
    pub number: u32,
    #[serde(rename = "Created")]
    pub created: String,
}

type Bits = Vec<Bit>;

#[derive(Debug, Clone, Deserialize)]
struct Subgift {
    // pub id: u16,
    pub user_id: Option<u64>,
    pub number: u16,
    pub tier: String,
    pub created_at: String,
}

type Subgifts = Vec<Subgift>;

#[derive(Debug, Clone, Deserialize)]
struct Followers {
    pub data: Vec<JsonUser>,
}

#[derive(Debug, Clone, Deserialize)]
struct JsonUser {
    pub followed_at: String,
    pub user_id: String,
    pub user_name: String,
}

fn get_users() -> Result<Users, Box<dyn Error>> {
    let mut users: Users = vec![];
    let mut rdr = csv::Reader::from_path("users-abc.csv")?;

    for result in rdr.deserialize() {
        let record: User = result?;
        users.push(record);
    }

    Ok(users)
}

fn get_bits() -> Result<Bits, Box<dyn Error>> {
    let mut bits: Bits = vec![];
    let mut rdr = csv::Reader::from_path("bits-most_recent_bits.csv")?;

    for result in rdr.deserialize() {
        let record: Bit = result?;
        bits.push(record);
    }

    Ok(bits)
}

fn get_subgifts() -> Result<Subgifts, Box<dyn Error>> {
    let mut subgifts: Subgifts = vec![];
    let mut rdr = csv::Reader::from_path("subgifts-most_recent_subgift.csv")?;

    for result in rdr.deserialize() {
        let record: Subgift = result?;
        subgifts.push(record);
    }

    Ok(subgifts)
}

fn get_json_users() -> Result<Vec<JsonUser>, Box<dyn Error>> {
    let raw = std::fs::read_to_string("../followers.json");
    if raw.is_err() {
        return Ok([].to_vec());
    }
    let parsed =
        serde_json::from_str::<Followers>(&raw.unwrap()).expect("Failed to parse followers");

    Ok(parsed.data)
}

fn string_time_to_iso(raw: String) -> String {
    let fmt = "%Y-%m-%d %H:%M";
    let dt = NaiveDateTime::parse_from_str(&raw, fmt).expect("Failed to parse time");
    let amsterdam_time = Amsterdam.from_local_datetime(&dt).unwrap();
    let utc: DateTime<Utc> = amsterdam_time.with_timezone(&Utc);

    format!("{}", utc.format("%+")).to_string()
}

impl From<User> for user::User {
    fn from(value: User) -> Self {
        let mut bld = user::User::builder(value.display_name, value.twitch_id)
            .created_at(string_time_to_iso(value.created_at));

        if let Some(follow_since) = value.follower_since {
            bld = bld.clone().follow(string_time_to_iso(follow_since));
        }

        if let (Some(sub_since), Some(tier)) = (value.subscriber_since, value.subscription_tier) {
            bld = bld.clone().subscribe(string_time_to_iso(sub_since));
            bld = bld.clone().tier(tier);
        }

        // using the old id to map it at create time
        let mut user = bld.build();
        user.id = value.id;
        user
    }
}

impl From<Bit> for bits::Bit {
    fn from(value: Bit) -> Self {
        let created_at = string_time_to_iso(value.created);
        Self {
            id: 0,
            user_id: value.user_id,
            number: value.number,
            created_at,
            message: value.message,
        }
    }
}

impl From<Subgift> for subgifts::Subgift {
    fn from(value: Subgift) -> Self {
        Self {
            id: 0,
            user_id: value.user_id,
            number: value.number,
            tier: value.tier,
            created_at: string_time_to_iso(value.created_at),
        }
    }
}

async fn get_connection(dev_mode: bool) -> Result<libsql::Connection, Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let local_path = "import.sqlite".to_string();
    let db_url = std::env::var("NOST_TURSO_DB_URL").unwrap();
    let db_token = std::env::var("NOST_TURSO_AUTH_TOKEN").unwrap();

    let db = match dev_mode {
        true => libsql::Builder::new_local(local_path).build().await?,
        false => {
            libsql::Builder::new_remote(db_url, db_token)
                .build()
                .await?
        }
    };

    Ok(db.connect().expect("Unable to connect to database"))
}

async fn import_from_csvs(conn: &libsql::Connection) -> Result<(), Box<dyn Error>> {
    let users = get_users()?;
    let subgifts = get_subgifts()?;
    let bits = get_bits()?;

    // used to map new ids to old records (for hard deleted rows)
    // let mut table_users: Vec<user::User> = users.into_iter().map(|u| u.into()).collect();
    // table_users.sort_unstable_by_key(|r| (r.created_at.clone(), r.follower_since.clone()));
    let query = "select * from users order by id asc".to_string();
    let table_users: Vec<user::User> = tables::Orm::<user::User>::query(conn, &query, [].to_vec())
        .await
        .expect("Failed to get users");

    let mut table_subgifts: Vec<subgifts::Subgift> =
        subgifts.into_iter().map(|u| u.into()).collect();
    table_subgifts.sort_by_key(|r| r.created_at.clone());

    let mut table_bits: Vec<bits::Bit> = bits.into_iter().map(|u| u.into()).collect();
    table_bits.sort_by_key(|r| r.created_at.clone());

    let mut user_map = HashMap::<u64, u64>::new();
    let pb = ProgressBar::new(table_users.len() as u64);
    pb.set_position(1);
    for user in table_users.iter() {
        // let id = user.create(conn).await.expect("Failed to insert user");
        let st_user = users.clone().into_iter().find(|u| user.id == u.id);
        if let Some(st_user) = st_user {
            user_map.insert(user.id, st_user.id);
        }
        pb.inc(1);
    }
    pb.finish();

    let pb = ProgressBar::new(table_subgifts.len() as u64);
    pb.set_position(1);
    for subgift in table_subgifts.iter_mut() {
        if let Some(user_id) = subgift.user_id {
            if let Some(mapped_user_id) = user_map.get(&user_id) {
                subgift.user_id = Some(*mapped_user_id);
            }
        }

        let mut columns: Vec<String> = vec![
            "number".to_string(),
            "tier".to_string(),
            "created_at".to_string(),
        ];
        let mut replacements = vec![
            subgift.number.to_string(),
            subgift.tier.clone(),
            subgift.created_at.clone(),
        ];

        if let Some(id) = subgift.user_id {
            columns.push("user_id".to_string());
            replacements.push(id.to_string());
        }

        let query = format!(
            "insert into subgifts (
                {}
            ) values (
                {}
            )",
            columns.join(", "),
            Orm::<Subgift>::placeholders(columns.len()),
        );

        let _ = Orm::<()>::query(conn, &query, replacements)
            .await
            .expect("Failed to insert subgift");

        pb.inc(1);
    }
    pb.finish();

    let pb = ProgressBar::new(table_bits.len() as u64);
    pb.set_position(1);
    for bit in table_bits.iter_mut() {
        if let Some(user_id) = bit.user_id {
            if let Some(mapped_user_id) = user_map.get(&user_id) {
                bit.user_id = Some(*mapped_user_id);
            }
        }

        let mut columns: Vec<String> = vec!["number".to_string(), "created_at".to_string()];
        let mut replacements = vec![bit.number.to_string(), bit.created_at.clone()];

        if let Some(id) = bit.user_id {
            columns.push("user_id".to_string());
            replacements.push(id.to_string());
        }

        if let Some(message) = &bit.message {
            columns.push("message".to_string());
            replacements.push(message.to_string());
        }

        let query = format!(
            "insert into bits (
                {}
            ) values (
                {}
            )",
            columns.join(", "),
            Orm::<Subgift>::placeholders(columns.len()),
        );

        let _ = Orm::<()>::query(conn, &query, replacements)
            .await
            .expect("Failed to insert bit");

        pb.inc(1);
    }
    pb.finish();

    Ok(())
}

async fn import_followers_json(conn: &libsql::Connection) -> Result<(), Box<dyn Error>> {
    let mut followers = get_json_users()?;
    let query = "select * from users where deleted_at is null".to_string();
    let mut table_users = tables::Orm::<user::User>::query(conn, &query, [].to_vec())
        .await
        .expect("Failed to retrieve db users");

    let pb = ProgressBar::new(table_users.len() as u64);
    pb.set_position(1);
    for user in table_users.iter_mut() {
        let follower = followers.clone().into_iter().find(|f| {
            let twitch_id = f.user_id.parse::<u64>().expect("Invalid u64 user_id");
            twitch_id == user.twitch_id
        });

        match follower {
            None => user.follower_since = None,
            Some(follower) => user.follower_since = Some(follower.followed_at),
        }

        user.update(conn).await.expect("Failed to update user");
        pb.inc(1);
    }
    pb.finish();

    let pb = ProgressBar::new(followers.len() as u64);
    pb.set_position(1);
    for follower in followers.iter_mut() {
        let twitch_id = follower
            .user_id
            .parse::<u64>()
            .expect("Invalid u64 user_id");
        let user = table_users
            .clone()
            .into_iter()
            .find(|u| u.twitch_id == twitch_id);

        match user {
            None => {
                let new_user = user::User::builder(follower.clone().user_name, twitch_id)
                    .follow(follower.clone().followed_at)
                    .build();
                new_user.create(conn).await.expect("Failed to create user");
            }
            Some(mut user) => {
                user.follower_since = Some(follower.clone().followed_at);
                user.update(conn).await.expect("Failed to update user");
            }
        }
        pb.inc(1);
    }
    pb.finish();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let conn = get_connection(true).await?;

    import_from_csvs(&conn).await?;
    import_followers_json(&conn).await?;

    Ok(())
}
