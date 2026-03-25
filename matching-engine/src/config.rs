use std::env;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub orders_topic: String,
    pub fills_topic: String,
    pub consumer_group: String,
    pub database_url: String,
    pub wal_path: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            kafka_brokers: env::var("KAFKA_BROKERS").context("KAFKA_BROKERS is required")?,
            orders_topic: env::var("ORDERS_TOPIC").unwrap_or_else(|_| "orders".to_string()),
            fills_topic: env::var("FILLS_TOPIC").unwrap_or_else(|_| "fills".to_string()),
            consumer_group: env::var("MATCHING_CONSUMER_GROUP")
                .unwrap_or_else(|_| "matching-engine".to_string()),
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            wal_path: env::var("WAL_PATH").unwrap_or_else(|_| "./wal.log".to_string()),
        })
    }
}
