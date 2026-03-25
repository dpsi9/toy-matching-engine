use std::{sync::Arc, time::Duration};

use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    web, HttpResponse, Result,
};
use chrono::Utc;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use shared::models::{Order, OrderBookView, OrderCommand, OrderType, PriceLevelView, Side};

#[derive(Clone)]
pub struct AppState {
    pub producer: Arc<FutureProducer>,
    pub orders_topic: String,
    pub db: PgPool,
    pub orderbook_query_limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub market_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<u64>,
    pub qty: u64,
}

#[derive(Debug, Serialize)]
pub struct CreateOrderResponse {
    pub order_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    pub market_id: String,
    pub order_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct OrderBookQuery {
    pub market_id: String,
}

pub async fn create_order(
    state: web::Data<AppState>,
    body: web::Json<CreateOrderRequest>,
) -> Result<HttpResponse> {
    if body.market_id.trim().is_empty() {
        return Err(ErrorBadRequest("market_id is required"));
    }
    if body.qty == 0 {
        return Err(ErrorBadRequest("qty must be > 0"));
    }

    let order_id_i64 = sqlx::query_scalar::<_, i64>("SELECT nextval('order_id_seq')")
        .fetch_one(&state.db)
        .await
        .map_err(ErrorInternalServerError)?;
    let order_id = order_id_i64 as u64;
    let command_id = Uuid::new_v4().to_string();
    let order = match body.order_type {
        OrderType::Limit => {
            let price = body
                .price
                .ok_or_else(|| ErrorBadRequest("price is required for limit orders"))?;
            if price == 0 {
                return Err(ErrorBadRequest("price must be > 0 for limit orders"));
            }
            Order::new_limit(order_id, body.market_id.clone(), body.side, price, body.qty)
        }
        OrderType::Market => {
            Order::new_market(order_id, body.market_id.clone(), body.side, body.qty)
        }
    };

    let payload = serde_json::to_string(&OrderCommand::Submit { command_id, order })
        .map_err(ErrorInternalServerError)?;

    state
        .producer
        .send(
            FutureRecord::to(&state.orders_topic)
                .key(&body.market_id)
                .payload(&payload),
            Duration::from_secs(5),
        )
        .await
        .map_err(|(e, _)| ErrorInternalServerError(e.to_string()))?;

    Ok(HttpResponse::Accepted().json(CreateOrderResponse { order_id }))
}

pub async fn cancel_order(
    state: web::Data<AppState>,
    body: web::Json<CancelRequest>,
) -> Result<HttpResponse> {
    if body.market_id.trim().is_empty() {
        return Err(ErrorBadRequest("market_id is required"));
    }

    let command_id = Uuid::new_v4().to_string();
    let payload = serde_json::to_string(&OrderCommand::Cancel {
        command_id,
        market_id: body.market_id.clone(),
        order_id: body.order_id,
        timestamp_ms: Utc::now().timestamp_millis(),
    })
    .map_err(ErrorInternalServerError)?;

    state
        .producer
        .send(
            FutureRecord::to(&state.orders_topic)
                .key(&body.market_id)
                .payload(&payload),
            Duration::from_secs(5),
        )
        .await
        .map_err(|(e, _)| ErrorInternalServerError(e.to_string()))?;

    Ok(HttpResponse::Accepted().finish())
}

pub async fn orderbook(
    state: web::Data<AppState>,
    query: web::Query<OrderBookQuery>,
) -> Result<HttpResponse> {
    if query.market_id.trim().is_empty() {
        return Err(ErrorBadRequest("market_id is required"));
    }

    let bids_rows = sqlx::query_as::<_, (i64, i64)>(
        "
        SELECT price::BIGINT, SUM(remaining_qty)::BIGINT AS qty
        FROM orders
        WHERE market_id = $1
          AND side = 'buy'
          AND remaining_qty > 0
          AND status IN ('open', 'partially_filled')
        GROUP BY price
        ORDER BY price DESC
        LIMIT $2
        ",
    )
    .bind(&query.market_id)
    .bind(state.orderbook_query_limit)
    .fetch_all(&state.db)
    .await
    .map_err(ErrorInternalServerError)?;

    let asks_rows = sqlx::query_as::<_, (i64, i64)>(
        "
        SELECT price::BIGINT, SUM(remaining_qty)::BIGINT AS qty
        FROM orders
        WHERE market_id = $1
          AND side = 'sell'
          AND remaining_qty > 0
          AND status IN ('open', 'partially_filled')
        GROUP BY price
        ORDER BY price ASC
        LIMIT $2
        ",
    )
    .bind(&query.market_id)
    .bind(state.orderbook_query_limit)
    .fetch_all(&state.db)
    .await
    .map_err(ErrorInternalServerError)?;

    let response = OrderBookView {
        market_id: query.market_id.clone(),
        bids: bids_rows
            .into_iter()
            .map(|(price, qty)| PriceLevelView {
                price: price as u64,
                qty: qty as u64,
            })
            .collect(),
        asks: asks_rows
            .into_iter()
            .map(|(price, qty)| PriceLevelView {
                price: price as u64,
                qty: qty as u64,
            })
            .collect(),
    };

    Ok(HttpResponse::Ok().json(response))
}
