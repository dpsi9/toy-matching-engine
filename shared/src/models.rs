use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Order {
    pub id: u64,
    pub side: Side,
    pub price: u64,
    pub qty: u64,
    pub market_id: String,
    pub timestamp_ms: i64,
    pub status: OrderStatus,
    pub remaining_qty: u64,
    pub order_type: OrderType,
}

impl Order {
    pub fn new_limit(id: u64, market_id: String, side: Side, price: u64, qty: u64) -> Self {
        Self {
            id,
            side,
            price,
            qty,
            market_id,
            timestamp_ms: Utc::now().timestamp_millis(),
            status: OrderStatus::Open,
            remaining_qty: qty,
            order_type: OrderType::Limit,
        }
    }

    pub fn new_market(id: u64, market_id: String, side: Side, qty: u64) -> Self {
        Self {
            id,
            side,
            price: 0,
            qty,
            market_id,
            timestamp_ms: Utc::now().timestamp_millis(),
            status: OrderStatus::Open,
            remaining_qty: qty,
            order_type: OrderType::Market,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fill {
    pub maker_order_id: u64,
    pub taker_order_id: u64,
    pub price: u64,
    pub qty: u64,
    pub market_id: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrderCommand {
    Submit {
        command_id: String,
        order: Order,
    },
    Cancel {
        command_id: String,
        market_id: String,
        order_id: u64,
        timestamp_ms: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevelView {
    pub price: u64,
    pub qty: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookView {
    pub market_id: String,
    pub bids: Vec<PriceLevelView>,
    pub asks: Vec<PriceLevelView>,
}
