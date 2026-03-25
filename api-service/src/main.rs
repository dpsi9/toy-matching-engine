mod config;
mod http;
mod ws;

use std::sync::Arc;

use actix_web::{middleware::Logger, web, App, HttpServer};
use anyhow::Result;
use dotenvy::dotenv;
use rdkafka::Message;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tracing::{error, info};

use config::Config;
use http::{cancel_order, create_order, orderbook, AppState};
use shared::{kafka, models::Fill};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    init_tracing();

    let config = Config::from_env()?;
    let producer = Arc::new(kafka::create_producer(&config.kafka_brokers)?);
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    let (fills_tx, _) = broadcast::channel::<Fill>(4096);
    spawn_fill_fanout(
        config.kafka_brokers.clone(),
        config.ws_consumer_group.clone(),
        config.fills_topic.clone(),
        fills_tx.clone(),
    );

    let app_state = AppState {
        producer,
        orders_topic: config.orders_topic.clone(),
        db,
        orderbook_query_limit: config.orderbook_query_limit,
    };

    let bind_addr = config.bind_addr.clone();
    info!(%bind_addr, "starting api-service");

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(fills_tx.clone()))
            .route("/orders", web::post().to(create_order))
            .route("/cancel", web::post().to(cancel_order))
            .route("/orderbook", web::get().to(orderbook))
            .route("/ws", web::get().to(ws::ws_handler))
    })
    .bind(&config.bind_addr)?
    .run()
    .await?;

    Ok(())
}

fn spawn_fill_fanout(
    brokers: String,
    group_id: String,
    fills_topic: String,
    tx: broadcast::Sender<Fill>,
) {
    tokio::spawn(async move {
        let consumer =
            match kafka::create_consumer(&brokers, &group_id, &[&fills_topic], "latest", true) {
                Ok(c) => c,
                Err(err) => {
                    error!(error = %err, "failed to create fill consumer");
                    return;
                }
            };

        loop {
            match consumer.recv().await {
                Err(err) => {
                    error!(error = %err, "kafka consume error on fills topic");
                }
                Ok(msg) => {
                    if let Some(payload) = msg.payload() {
                        match serde_json::from_slice::<Fill>(payload) {
                            Ok(fill) => {
                                let _ = tx.send(fill);
                            }
                            Err(err) => {
                                error!(error = %err, "invalid fill payload");
                            }
                        }
                    }
                }
            }
        }
    });
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,api_service=debug,actix_web=info".to_string()),
        )
        .init();
}
