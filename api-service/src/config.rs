use std::env;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub kafka_brokers: String,
    pub orders_topic: String,
    pub fills_topic: String,
    pub ws_consumer_group: String,
    pub database_url: String,
    pub orderbook_query_limit: i64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            kafka_brokers: env::var("KAFKA_BROKERS").context("KAFKA_BROKERS is required")?,
            orders_topic: env::var("ORDERS_TOPIC").unwrap_or_else(|_| "orders".to_string()),
            fills_topic: env::var("FILLS_TOPIC").unwrap_or_else(|_| "fills".to_string()),
            ws_consumer_group: env::var("WS_CONSUMER_GROUP")
                .unwrap_or_else(|_| "api-service-fills".to_string()),
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            orderbook_query_limit: env::var("API_ORDERBOOK_QUERY_LIMIT")
                .ok()
                .and_then(|x| x.parse().ok())
                .unwrap_or(1000),
        })
    }
}
