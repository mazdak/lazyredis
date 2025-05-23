use redis::{aio::MultiplexedConnection, Client};
use crate::config::ConnectionProfile;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum RedisError {
    Client(redis::RedisError),
    Connection(String),
    Other(String),
}

impl fmt::Display for RedisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedisError::Client(e) => write!(f, "Redis client error: {}", e),
            RedisError::Connection(msg) => write!(f, "Connection error: {}", msg),
            RedisError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl Error for RedisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RedisError::Client(e) => Some(e),
            _ => None,
        }
    }
}

impl From<redis::RedisError> for RedisError {
    fn from(e: redis::RedisError) -> Self {
        RedisError::Client(e)
    }
}

pub struct RedisClient {
    pub client: Option<Client>,
    pub connection: Option<MultiplexedConnection>,
    pub db_index: usize,
    pub connection_status: String,
}

impl RedisClient {
    pub fn new() -> Self {
        Self {
            client: None,
            connection: None,
            db_index: 0,
            connection_status: String::from("Not connected"),
        }
    }

    pub async fn connect_to_profile(
        &mut self,
        profile: &ConnectionProfile,
        use_profile_db: bool,
        target_db_index_override: Option<usize>,
    ) -> Result<(), RedisError> {
        self.connection_status = format!("Connecting to {} ({})...", profile.name, profile.url);
        let client = Client::open(profile.url.as_str())?;
        self.client = Some(client);
        let mut connection = self
            .client
            .as_ref()
            .unwrap()
            .get_multiplexed_async_connection()
            .await?;
        let db_to_select = if use_profile_db {
            profile.db.unwrap_or(self.db_index as u8)
        } else {
            target_db_index_override.unwrap_or(self.db_index) as u8
        };
        redis::cmd("SELECT")
            .arg(db_to_select)
            .query_async::<()>(&mut connection)
            .await?;
        self.db_index = db_to_select as usize;
        self.connection = Some(connection);
        self.connection_status = format!(
            "Connected to {} ({}), DB {}",
            profile.name, profile.url, self.db_index
        );
        Ok(())
    }

    pub async fn fetch_keys(&mut self) -> Result<Vec<String>, RedisError> {
        let mut keys = Vec::new();
        if let Some(mut con) = self.connection.take() {
            let mut cursor: u64 = 0;
            loop {
                match redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg("*")
                    .arg("COUNT")
                    .arg(1000)
                    .query_async::<(u64, Vec<String>)>(&mut con)
                    .await
                {
                    Ok((next_cursor, batch)) => {
                        cursor = next_cursor;
                        keys.extend(batch);
                        if cursor == 0 {
                            break;
                        }
                    }
                    Err(e) => {
                        self.connection = Some(con);
                        return Err(RedisError::Client(e));
                    }
                }
            }
            self.connection = Some(con);
            Ok(keys)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available to fetch keys.".to_string(),
            ))
        }
    }

    pub async fn delete_prefix(&mut self, prefix: &str, delimiter: char) -> Result<usize, RedisError> {
        if let Some(mut con) = self.connection.clone() {
            let pattern = format!("{}{}", prefix, if prefix.ends_with(delimiter) { "*" } else { "*" });
            let mut keys_to_delete: Vec<String> = Vec::new();
            let mut cursor: u64 = 0;
            loop {
                match redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(&pattern)
                    .arg("COUNT")
                    .arg(100)
                    .query_async::<(u64, Vec<String>)>(&mut con)
                    .await
                {
                    Ok((next_cursor, batch)) => {
                        keys_to_delete.extend(batch);
                        if next_cursor == 0 {
                            break;
                        }
                        cursor = next_cursor;
                    }
                    Err(e) => return Err(RedisError::Client(e)),
                }
            }
            if keys_to_delete.is_empty() {
                return Ok(0);
            }
            let count = redis::cmd("DEL")
                .arg(keys_to_delete.as_slice())
                .query_async::<i32>(&mut con)
                .await?;
            Ok(count as usize)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available for deleting prefix.".to_string(),
            ))
        }
    }

    pub async fn delete_key(&mut self, key: &str) -> Result<bool, RedisError> {
        if let Some(mut con) = self.connection.clone() {
            let count = redis::cmd("DEL")
                .arg(key)
                .query_async::<i32>(&mut con)
                .await?;
            Ok(count > 0)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available for deleting key.".to_string(),
            ))
        }
    }

    pub async fn get_key_type(&mut self, key: &str) -> Result<String, RedisError> {
        if let Some(mut con) = self.connection.clone() {
            let key_type = redis::cmd("TYPE")
                .arg(key)
                .query_async::<String>(&mut con)
                .await?;
            Ok(key_type)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available for key type.".to_string(),
            ))
        }
    }

    pub async fn get_ttl(&mut self, key: &str) -> Result<i64, RedisError> {
        if let Some(mut con) = self.connection.clone() {
            let ttl = redis::cmd("TTL")
                .arg(key)
                .query_async::<i64>(&mut con)
                .await?;
            Ok(ttl)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available for TTL.".to_string(),
            ))
        }
    }

    pub async fn get_string(&mut self, key: &str) -> Result<Option<String>, RedisError> {
        if let Some(mut con) = self.connection.clone() {
            let value = redis::cmd("GET")
                .arg(key)
                .query_async::<Option<String>>(&mut con)
                .await?;
            Ok(value)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available for getting string.".to_string(),
            ))
        }
    }

    pub async fn get_info(&mut self) -> Result<String, RedisError> {
        if let Some(mut con) = self.connection.clone() {
            let info = redis::cmd("INFO")
                .query_async::<String>(&mut con)
                .await?;
            Ok(info)
        } else {
            Err(RedisError::Connection(
                "No Redis connection available for INFO command.".to_string(),
            ))
        }
    }

    // Add more methods for hash, list, set, zset, stream as needed
} 