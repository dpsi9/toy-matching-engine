use matching_engine::engine::Engine;
use shared::models::{Order, OrderStatus, Side};

#[test]
fn order_matching_full_fill() {
    let mut engine = Engine::new();

    let maker = Order::new_limit(1, "m1".to_string(), Side::Sell, 100, 10);
    let result1 = engine.submit_order(maker);
    assert!(result1.fills.is_empty());

    let taker = Order::new_limit(2, "m1".to_string(), Side::Buy, 100, 10);
    let result2 = engine.submit_order(taker);

    assert_eq!(result2.fills.len(), 1);
    let fill = &result2.fills[0];
    assert_eq!(fill.maker_order_id, 1);
    assert_eq!(fill.taker_order_id, 2);
    assert_eq!(fill.qty, 10);

    let maker_update = result2
        .updates
        .iter()
        .find(|u| u.order_id == 1)
        .expect("maker update");
    let taker_update = result2
        .updates
        .iter()
        .find(|u| u.order_id == 2)
        .expect("taker update");

    assert_eq!(maker_update.status, OrderStatus::Filled);
    assert_eq!(taker_update.status, OrderStatus::Filled);
}

#[test]
fn partial_fill_then_resting_qty() {
    let mut engine = Engine::new();

    let maker = Order::new_limit(10, "m2".to_string(), Side::Sell, 100, 10);
    engine.submit_order(maker);

    let taker = Order::new_limit(11, "m2".to_string(), Side::Buy, 100, 15);
    let result = engine.submit_order(taker);

    assert_eq!(result.fills.len(), 1);
    assert_eq!(result.fills[0].qty, 10);

    let taker_update = result
        .updates
        .iter()
        .find(|u| u.order_id == 11)
        .expect("taker update");
    assert_eq!(taker_update.remaining_qty, 5);
    assert_eq!(taker_update.status, OrderStatus::PartiallyFilled);
}

#[test]
fn cancel_order_removes_resting_order() {
    let mut engine = Engine::new();

    let order = Order::new_limit(20, "m3".to_string(), Side::Buy, 90, 7);
    engine.submit_order(order);

    let cancelled = engine.cancel_order("m3", 20).expect("cancelled");
    assert_eq!(cancelled.status, OrderStatus::Cancelled);
    assert_eq!(cancelled.remaining_qty, 7);

    assert!(engine.cancel_order("m3", 20).is_none());
}
