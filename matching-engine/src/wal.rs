use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};

use shared::models::Order;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WalEvent {
    OrderReceived { order: Order },
    TradeExecuted {
        market_id: String,
        maker_order_id: u64,
        taker_order_id: u64,
        price: u64,
        qty: u64,
        timestamp_ms: i64,
    },
    Cancel {
        market_id: String,
        order_id: u64,
        timestamp_ms: i64,
    },
}

#[derive(Debug, Clone)]
pub struct Wal {
    path: String,
}

impl Wal {
    pub fn new(path: String) -> Self {
        Self { path }
    }

    pub async fn append(&self, event: &WalEvent) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        let line = serde_json::to_string(event)?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn replay(&self) -> Result<Vec<WalEvent>> {
        let file = match OpenOptions::new().read(true).open(&self.path).await {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };

        let mut events = Vec::new();
        let mut lines = BufReader::new(file).lines();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<WalEvent>(&line) {
                events.push(event);
            }
        }

        Ok(events)
    }
}
