use std::collections::{BTreeMap, HashMap, VecDeque};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use shared::models::{Fill, Order, OrderStatus, OrderType, Side};

use crate::wal::WalEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub order_id: u64,
    pub market_id: String,
    pub remaining_qty: u64,
    pub status: OrderStatus,
}

#[derive(Debug, Default)]
pub struct Engine {
    books: HashMap<String, OrderBook>,
}

#[derive(Debug, Default)]
struct OrderBook {
    bids: BTreeMap<u64, PriceLevel>,
    asks: BTreeMap<u64, PriceLevel>,
}

#[derive(Debug, Default)]
struct PriceLevel {
    orders: VecDeque<Order>,
}

#[derive(Debug, Default)]
pub struct MatchResult {
    pub fills: Vec<Fill>,
    pub updates: Vec<OrderUpdate>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit_order(&mut self, order: Order) -> MatchResult {
        let market_id = order.market_id.clone();
        let book = self.books.entry(market_id).or_default();
        book.submit(order)
    }

    pub fn cancel_order(&mut self, market_id: &str, order_id: u64) -> Option<OrderUpdate> {
        self.books
            .get_mut(market_id)
            .and_then(|book| book.cancel(order_id, market_id))
    }

    pub fn apply_wal_event(&mut self, event: &WalEvent) {
        match event {
            WalEvent::OrderReceived { order } => {
                let _ = self.submit_order(order.clone());
            }
            WalEvent::Cancel {
                market_id, order_id, ..
            } => {
                let _ = self.cancel_order(market_id, *order_id);
            }
            WalEvent::TradeExecuted {
                market_id,
                maker_order_id,
                taker_order_id,
                qty,
                ..
            } => {
                if let Some(book) = self.books.get_mut(market_id) {
                    let _ = book.reduce_order(*maker_order_id, *qty, market_id);
                    let _ = book.reduce_order(*taker_order_id, *qty, market_id);
                }
            }
        }
    }
}

impl OrderBook {
    fn submit(&mut self, mut taker: Order) -> MatchResult {
        let now = Utc::now().timestamp_millis();
        let taker_original_qty = taker.qty;
        let mut fills = Vec::new();
        let mut updates: HashMap<u64, OrderUpdate> = HashMap::new();

        match taker.side {
            Side::Buy => {
                self.match_buy_order(&mut taker, now, &mut fills, &mut updates);
            }
            Side::Sell => {
                self.match_sell_order(&mut taker, now, &mut fills, &mut updates);
            }
        }

        if taker.remaining_qty > 0 && taker.order_type == OrderType::Limit {
            let status = if taker.remaining_qty == taker_original_qty {
                OrderStatus::Open
            } else {
                OrderStatus::PartiallyFilled
            };
            taker.status = status;
            self.insert_resting_order(taker.clone());
            updates.insert(
                taker.id,
                OrderUpdate {
                    order_id: taker.id,
                    market_id: taker.market_id.clone(),
                    remaining_qty: taker.remaining_qty,
                    status,
                },
            );
        } else {
            let status = if taker.remaining_qty == 0 {
                OrderStatus::Filled
            } else if taker.remaining_qty < taker_original_qty {
                OrderStatus::PartiallyFilled
            } else {
                OrderStatus::Cancelled
            };

            updates.insert(
                taker.id,
                OrderUpdate {
                    order_id: taker.id,
                    market_id: taker.market_id.clone(),
                    remaining_qty: taker.remaining_qty,
                    status,
                },
            );
        }

        MatchResult {
            fills,
            updates: updates.into_values().collect(),
        }
    }

    fn match_buy_order(
        &mut self,
        taker: &mut Order,
        now: i64,
        fills: &mut Vec<Fill>,
        updates: &mut HashMap<u64, OrderUpdate>,
    ) {
        while taker.remaining_qty > 0 {
            let best_ask = match self.asks.keys().next().copied() {
                Some(price) => price,
                None => break,
            };

            if taker.order_type == OrderType::Limit && best_ask > taker.price {
                break;
            }

            let mut level = self.asks.remove(&best_ask).unwrap_or_default();
            while taker.remaining_qty > 0 {
                let Some(mut maker) = level.orders.pop_front() else {
                    break;
                };

                let traded_qty = taker.remaining_qty.min(maker.remaining_qty);
                taker.remaining_qty -= traded_qty;
                maker.remaining_qty -= traded_qty;

                fills.push(Fill {
                    maker_order_id: maker.id,
                    taker_order_id: taker.id,
                    price: best_ask,
                    qty: traded_qty,
                    market_id: taker.market_id.clone(),
                    timestamp_ms: now,
                });

                let maker_status = if maker.remaining_qty == 0 {
                    OrderStatus::Filled
                } else {
                    OrderStatus::PartiallyFilled
                };
                updates.insert(
                    maker.id,
                    OrderUpdate {
                        order_id: maker.id,
                        market_id: maker.market_id.clone(),
                        remaining_qty: maker.remaining_qty,
                        status: maker_status,
                    },
                );

                if maker.remaining_qty > 0 {
                    maker.status = OrderStatus::PartiallyFilled;
                    level.orders.push_front(maker);
                }

                if taker.remaining_qty == 0 {
                    break;
                }
            }

            if !level.orders.is_empty() {
                self.asks.insert(best_ask, level);
            }
        }
    }

    fn match_sell_order(
        &mut self,
        taker: &mut Order,
        now: i64,
        fills: &mut Vec<Fill>,
        updates: &mut HashMap<u64, OrderUpdate>,
    ) {
        while taker.remaining_qty > 0 {
            let best_bid = match self.bids.keys().next_back().copied() {
                Some(price) => price,
                None => break,
            };

            if taker.order_type == OrderType::Limit && best_bid < taker.price {
                break;
            }

            let mut level = self.bids.remove(&best_bid).unwrap_or_default();
            while taker.remaining_qty > 0 {
                let Some(mut maker) = level.orders.pop_front() else {
                    break;
                };

                let traded_qty = taker.remaining_qty.min(maker.remaining_qty);
                taker.remaining_qty -= traded_qty;
                maker.remaining_qty -= traded_qty;

                fills.push(Fill {
                    maker_order_id: maker.id,
                    taker_order_id: taker.id,
                    price: best_bid,
                    qty: traded_qty,
                    market_id: taker.market_id.clone(),
                    timestamp_ms: now,
                });

                let maker_status = if maker.remaining_qty == 0 {
                    OrderStatus::Filled
                } else {
                    OrderStatus::PartiallyFilled
                };
                updates.insert(
                    maker.id,
                    OrderUpdate {
                        order_id: maker.id,
                        market_id: maker.market_id.clone(),
                        remaining_qty: maker.remaining_qty,
                        status: maker_status,
                    },
                );

                if maker.remaining_qty > 0 {
                    maker.status = OrderStatus::PartiallyFilled;
                    level.orders.push_front(maker);
                }

                if taker.remaining_qty == 0 {
                    break;
                }
            }

            if !level.orders.is_empty() {
                self.bids.insert(best_bid, level);
            }
        }
    }

    fn insert_resting_order(&mut self, order: Order) {
        let levels = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        levels.entry(order.price).or_default().orders.push_back(order);
    }

    fn cancel(&mut self, order_id: u64, market_id: &str) -> Option<OrderUpdate> {
        if let Some(update) = Self::cancel_from_levels(&mut self.bids, order_id, market_id) {
            return Some(update);
        }
        Self::cancel_from_levels(&mut self.asks, order_id, market_id)
    }

    fn cancel_from_levels(
        levels: &mut BTreeMap<u64, PriceLevel>,
        order_id: u64,
        market_id: &str,
    ) -> Option<OrderUpdate> {
        let mut price_to_remove = None;
        let mut result = None;

        for (price, level) in levels.iter_mut() {
            if let Some(pos) = level.orders.iter().position(|o| o.id == order_id) {
                let mut order = level.orders.remove(pos)?;
                order.status = OrderStatus::Cancelled;

                if level.orders.is_empty() {
                    price_to_remove = Some(*price);
                }

                result = Some(OrderUpdate {
                    order_id,
                    market_id: market_id.to_string(),
                    remaining_qty: order.remaining_qty,
                    status: OrderStatus::Cancelled,
                });
                break;
            }
        }

        if let Some(price) = price_to_remove {
            levels.remove(&price);
        }

        result
    }

    fn reduce_order(&mut self, order_id: u64, qty: u64, market_id: &str) -> Option<OrderUpdate> {
        if let Some(update) = Self::reduce_from_levels(&mut self.bids, order_id, qty, market_id) {
            return Some(update);
        }
        Self::reduce_from_levels(&mut self.asks, order_id, qty, market_id)
    }

    fn reduce_from_levels(
        levels: &mut BTreeMap<u64, PriceLevel>,
        order_id: u64,
        qty: u64,
        market_id: &str,
    ) -> Option<OrderUpdate> {
        let mut price_to_remove = None;
        let mut result = None;

        for (price, level) in levels.iter_mut() {
            if let Some(pos) = level.orders.iter().position(|o| o.id == order_id) {
                let order = level.orders.get_mut(pos)?;
                order.remaining_qty = order.remaining_qty.saturating_sub(qty);
                order.status = if order.remaining_qty == 0 {
                    OrderStatus::Filled
                } else {
                    OrderStatus::PartiallyFilled
                };

                let remaining_qty = order.remaining_qty;
                let status = order.status;

                if order.remaining_qty == 0 {
                    level.orders.remove(pos);
                }

                if level.orders.is_empty() {
                    price_to_remove = Some(*price);
                }

                result = Some(OrderUpdate {
                    order_id,
                    market_id: market_id.to_string(),
                    remaining_qty,
                    status,
                });
                break;
            }
        }

        if let Some(price) = price_to_remove {
            levels.remove(&price);
        }

        result
    }
}
