use pretty_assertions::assert_eq;
use standalone_engine::engine::{orderbook_row, trade_row, Engine};
use standalone_engine::types::{BookSide, MarketInfo, OrderSide, TradeSide};
use std::collections::HashMap;

#[test]
fn snapshot_finish_and_pending_adjust() {
    let mut markets = HashMap::new();
    markets.insert(
        1,
        MarketInfo {
            tick_size: 0.1,
            contract_value: 1.0,
        },
    );
    let mut eng = Engine::new(markets);

    // snapshot
    eng.on_orderbook(orderbook_row(1, 1, true, BookSide::Bid, 100.0, 10.0))
        .unwrap();
    eng.on_orderbook(orderbook_row(1, 1, true, BookSide::Ask, 101.0, 12.0))
        .unwrap();

    // first incremental -> finish snapshot
    eng.on_orderbook(orderbook_row(2, 1, false, BookSide::Bid, 100.0, 10.0))
        .unwrap();

    let s = eng.summary(1).unwrap();
    assert_eq!(s.is_finish_snap, true);
    assert_eq!(s.bid1, 100.0);
    assert_eq!(s.ask1, 101.0);

    eng.add_own_order(
        3,
        1,
        "o-bid-1".to_string(),
        OrderSide::Buy,
        100.0,
        1.0,
        "manual".to_string(),
        "manual".to_string(),
        1,
    )
    .unwrap();

    // increase bid amount at price -> delta positive -> backlen increases
    eng.on_orderbook(orderbook_row(4, 1, false, BookSide::Bid, 100.0, 12.0))
        .unwrap();

    let pending = eng
        .state
        .pending_bids
        .get(&1)
        .unwrap()
        .by_price
        .get(&ordered_float::OrderedFloat(100.0))
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tlen, 12.0);
    assert_eq!(pending[0].backlen, 2.0);
}

#[test]
fn trade_fills_after_queue_position() {
    let mut markets = HashMap::new();
    markets.insert(
        1,
        MarketInfo {
            tick_size: 0.1,
            contract_value: 1.0,
        },
    );
    let mut eng = Engine::new(markets);

    // snapshot book with ask=101 amount=10
    eng.on_orderbook(orderbook_row(1, 1, true, BookSide::Ask, 101.0, 10.0))
        .unwrap();
    eng.on_orderbook(orderbook_row(2, 1, false, BookSide::Ask, 101.0, 10.0))
        .unwrap();

    // our ask at same price, inpos=10
    eng.add_own_order(
        3,
        1,
        "o-ask-1".to_string(),
        OrderSide::Sell,
        101.0,
        1.0,
        "manual".to_string(),
        "manual".to_string(),
        1,
    )
    .unwrap();

    // trade buy amount 5 => consume queue only
    let fills = eng
        .on_trade(trade_row(4, 1, TradeSide::Buy, 101.0, 5.0))
        .unwrap();
    assert_eq!(fills.len(), 0);

    // trade buy amount 6 => remaining queue position 5 -> fill 1
    let fills = eng
        .on_trade(trade_row(5, 1, TradeSide::Buy, 101.0, 6.0))
        .unwrap();
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].client_order_id, "o-ask-1");
    assert_eq!(fills[0].filled, 1.0);
    assert_eq!(fills[0].is_full, true);
}

