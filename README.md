# Prediction Market Matching Engine

- `api-service`: HTTP API + WebSocket stream
- `matching-engine`: consumes orders, matches, writes DB/WAL
- `shared`: common models + Kafka helpers

Flow:

`API -> Kafka(orders) -> Matching Engine -> Kafka(fills) -> API WS`

## Run

```bash
docker compose up --build
```

## Endpoints

- `POST /orders`
- `POST /cancel`
- `GET /orderbook?market_id=...`
- `GET /ws`

## Required Q&A

### 1) How do multiple API instances avoid double matching?

API nodes only publish commands. Matching is centralized in `matching-engine` via Kafka consumer groups. Orders are keyed by `market_id`, so one market is processed in one ordered partition lane at a time. We also dedupe replays with `command_id` in `processed_commands`.

### 2) What order book data structure did you use and why?

Per market: bids/asks are `BTreeMap<price, level>`, and each level is a FIFO `VecDeque<Order>`. `BTreeMap` gives best price quickly and `VecDeque` preserves time priority at the same price.
I think here Slab should be used for fast indexing, or flat array indexing like HFT's.
### 3) What breaks first under real production load?

Usually a hot market partition becomes the first bottleneck (single ordered lane). After that, WAL growth/replay time and fill-consumer lag are likely pain points.

### 4) What would you build next in 4 hours?

I’d add an outbox for reliable fill publishing, improve replay/recovery tests, and add metrics (match latency, partition lag, dropped/late WS clients).
