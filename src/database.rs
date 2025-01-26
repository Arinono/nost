use libsql::{Connection, Result};
use std::sync::Arc;

use crate::env::Environment;

#[derive(Clone)]
pub enum Database {
    Local((Arc<libsql::Database>, Connection)),
    Remote(Arc<libsql::Database>),
}

impl Database {
    pub async fn new(env: &Environment) -> Result<Self> {
        if !env.dev_mode {
            let db = libsql::Builder::new_remote(
                env.turso_db_url.clone(),
                env.turso_auth_token.secret_str().to_string(),
            )
            .build()
            .await?;
            // let db = libsql::Builder::new_remote_replica(
            //     env.turso_local_db_path.clone(),
            //     env.turso_db_url.clone(),
            //     env.turso_auth_token.secret_str().to_string(),
            // )
            // .build()
            // .await?
            Ok(Self::Remote(Arc::new(db)))
        } else {
            let db = libsql::Builder::new_local(env.turso_local_db_path.clone())
                .build()
                .await?;
            let conn = db.connect()?;
            Ok(Self::Local((Arc::new(db), conn)))
        }
    }

    pub fn db(&self) -> Result<Arc<libsql::Database>> {
        let db = match self {
            Self::Local((db, _)) => db.clone(),
            Self::Remote(db) => db.clone(),
        };
        Ok(db)
    }

    pub fn conn(&self) -> Result<Connection> {
        let conn = match self {
            // It will always remain connected unless it encounters a major error.
            Self::Local((_, conn)) => conn.clone(),

            // For TCP-based remote libSQL, we establish a new database connection each time.
            // This is necessary because TCP cannot stay connected forever.
            Self::Remote(db) => db.connect()?,
        };
        Ok(conn)
    }
}
