CREATE TABLE IF NOT EXISTS markets (
    market_id TEXT PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS orders (
    id BIGINT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(market_id),
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    order_type TEXT NOT NULL CHECK (order_type IN ('limit', 'market')),
    price BIGINT NOT NULL,
    qty BIGINT NOT NULL,
    remaining_qty BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'partially_filled', 'filled', 'cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_orders_market_id ON orders(market_id);
CREATE INDEX IF NOT EXISTS idx_orders_market_side_price ON orders(market_id, side, price);
CREATE INDEX IF NOT EXISTS idx_orders_market_status ON orders(market_id, status);

CREATE TABLE IF NOT EXISTS fills (
    id BIGSERIAL PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(market_id),
    maker_order_id BIGINT NOT NULL REFERENCES orders(id),
    taker_order_id BIGINT NOT NULL REFERENCES orders(id),
    price BIGINT NOT NULL,
    qty BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_fills_market_id ON fills(market_id);
CREATE INDEX IF NOT EXISTS idx_fills_market_created_at ON fills(market_id, created_at DESC);
