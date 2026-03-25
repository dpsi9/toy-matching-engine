use std::time::Duration;

use anyhow::Result;
use dotenvy::dotenv;
use matching_engine::{
    config::Config,
    engine::Engine,
    persistence,
    wal::{Wal, WalEvent},
};
use rdkafka::{
    consumer::{CommitMode, Consumer},
    producer::FutureRecord,
    Message,
};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info, warn};

use shared::{kafka, models::OrderCommand};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    init_tracing();

    let config = Config::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let producer = kafka::create_producer(&config.kafka_brokers)?;
    let consumer = kafka::create_consumer(
        &config.kafka_brokers,
        &config.consumer_group,
        &[&config.orders_topic],
        "earliest",
        false,
    )?;

    let wal = Wal::new(config.wal_path.clone());
    let mut engine = Engine::new();

    for event in wal.replay().await? {
        engine.apply_wal_event(&event);
    }

    info!("matching-engine started");

    loop {
        let msg = consumer.recv().await?;
        let Some(payload) = msg.payload() else {
            continue;
        };

        let command = match serde_json::from_slice::<OrderCommand>(payload) {
            Ok(cmd) => cmd,
            Err(err) => {
                warn!(error = %err, "skipping malformed order command");
                continue;
            }
        };

        match command {
            OrderCommand::Submit { command_id, order } => {
                let mut tx = pool.begin().await?;
                let is_new = persistence::mark_command_processed(
                    &mut tx,
                    &command_id,
                    "submit",
                    Some(&order.market_id),
                )
                .await?;

                if !is_new {
                    tx.commit().await?;
                    if let Err(err) = consumer.commit_message(&msg, CommitMode::Async) {
                        warn!(error = %err, "failed to commit duplicate submit offset");
                    }
                    continue;
                }

                wal.append(&WalEvent::OrderReceived {
                    order: order.clone(),
                })
                .await?;

                let result = engine.submit_order(order.clone());

                persistence::insert_order(&mut tx, &order).await?;
                persistence::apply_updates(&mut tx, &result.updates).await?;
                persistence::insert_fills(&mut tx, &result.fills).await?;
                tx.commit().await?;

                for fill in result.fills {
                    wal.append(&WalEvent::TradeExecuted {
                        market_id: fill.market_id.clone(),
                        maker_order_id: fill.maker_order_id,
                        taker_order_id: fill.taker_order_id,
                        price: fill.price,
                        qty: fill.qty,
                        timestamp_ms: fill.timestamp_ms,
                    })
                    .await?;

                    let payload = serde_json::to_string(&fill)?;
                    if let Err((err, _msg)) = producer
                        .send(
                            FutureRecord::to(&config.fills_topic)
                                .key(&fill.market_id)
                                .payload(&payload),
                            Duration::from_secs(5),
                        )
                        .await
                    {
                        error!(error = %err, "failed to produce fill event");
                    }
                }
            }
            OrderCommand::Cancel {
                command_id,
                market_id,
                order_id,
                timestamp_ms,
            } => {
                let mut tx = pool.begin().await?;
                let is_new = persistence::mark_command_processed(
                    &mut tx,
                    &command_id,
                    "cancel",
                    Some(&market_id),
                )
                .await?;

                if !is_new {
                    tx.commit().await?;
                    if let Err(err) = consumer.commit_message(&msg, CommitMode::Async) {
                        warn!(error = %err, "failed to commit duplicate cancel offset");
                    }
                    continue;
                }

                wal.append(&WalEvent::Cancel {
                    market_id: market_id.clone(),
                    order_id,
                    timestamp_ms,
                })
                .await?;

                if let Some(update) = engine.cancel_order(&market_id, order_id) {
                    persistence::apply_updates(&mut tx, &[update]).await?;
                }

                tx.commit().await?;
            }
        }

        if let Err(err) = consumer.commit_message(&msg, CommitMode::Async) {
            warn!(error = %err, "failed to commit processed offset");
        }
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,matching_engine=debug,rdkafka=info".to_string()),
        )
        .init();
}
