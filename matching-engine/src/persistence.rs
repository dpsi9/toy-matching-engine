use anyhow::Result;
use sqlx::{Postgres, Transaction};

use shared::models::{Fill, Order, OrderStatus};

use crate::engine::OrderUpdate;

pub async fn mark_command_processed(
    tx: &mut Transaction<'_, Postgres>,
    command_id: &str,
    command_kind: &str,
    market_id: Option<&str>,
) -> Result<bool> {
    let result = sqlx::query(
        "
        INSERT INTO processed_commands (command_id, command_kind, market_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (command_id) DO NOTHING
        ",
    )
    .bind(command_id)
    .bind(command_kind)
    .bind(market_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() == 1)
}

pub async fn ensure_market(tx: &mut Transaction<'_, Postgres>, market_id: &str) -> Result<()> {
    sqlx::query(
        "
        INSERT INTO markets (market_id)
        VALUES ($1)
        ON CONFLICT (market_id) DO NOTHING
        ",
    )
    .bind(market_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn insert_order(tx: &mut Transaction<'_, Postgres>, order: &Order) -> Result<()> {
    ensure_market(tx, &order.market_id).await?;

    sqlx::query(
        "
        INSERT INTO orders (
            id, market_id, side, order_type, price, qty, remaining_qty, status, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, to_timestamp($9 / 1000.0), NOW())
        ON CONFLICT (id) DO NOTHING
        ",
    )
    .bind(order.id as i64)
    .bind(&order.market_id)
    .bind(match order.side {
        shared::models::Side::Buy => "buy",
        shared::models::Side::Sell => "sell",
    })
    .bind(match order.order_type {
        shared::models::OrderType::Limit => "limit",
        shared::models::OrderType::Market => "market",
    })
    .bind(order.price as i64)
    .bind(order.qty as i64)
    .bind(order.remaining_qty as i64)
    .bind(status_to_db(order.status))
    .bind(order.timestamp_ms)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn apply_updates(
    tx: &mut Transaction<'_, Postgres>,
    updates: &[OrderUpdate],
) -> Result<()> {
    for update in updates {
        sqlx::query(
            "
            UPDATE orders
            SET remaining_qty = $1,
                status = $2,
                updated_at = NOW()
            WHERE id = $3
            ",
        )
        .bind(update.remaining_qty as i64)
        .bind(status_to_db(update.status))
        .bind(update.order_id as i64)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn insert_fills(tx: &mut Transaction<'_, Postgres>, fills: &[Fill]) -> Result<()> {
    for fill in fills {
        sqlx::query(
            "
            INSERT INTO fills (
                market_id, maker_order_id, taker_order_id, price, qty, created_at
            )
            VALUES ($1, $2, $3, $4, $5, to_timestamp($6 / 1000.0))
            ",
        )
        .bind(&fill.market_id)
        .bind(fill.maker_order_id as i64)
        .bind(fill.taker_order_id as i64)
        .bind(fill.price as i64)
        .bind(fill.qty as i64)
        .bind(fill.timestamp_ms)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

fn status_to_db(status: OrderStatus) -> &'static str {
    match status {
        OrderStatus::Open => "open",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
    }
}
