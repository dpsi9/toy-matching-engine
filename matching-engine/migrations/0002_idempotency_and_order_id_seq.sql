CREATE SEQUENCE IF NOT EXISTS order_id_seq AS BIGINT START WITH 1 INCREMENT BY 1;

CREATE TABLE IF NOT EXISTS processed_commands (
    command_id TEXT PRIMARY KEY,
    command_kind TEXT NOT NULL,
    market_id TEXT,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_processed_commands_processed_at
    ON processed_commands(processed_at DESC);
