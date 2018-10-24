-- Your SQL goes here
CREATE TABLE strange_blockchain_transactions (
    hash VARCHAR PRIMARY KEY,
    from_ JSONB NOT NULL,
    to_ JSONB NOT NULL,
    block_number BIGINT NOT NULL,
    currency VARCHAR NOT NULL,
    value NUMERIC NOT NULL,
    fee NUMERIC NOT NULL,
    confirmations INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    commentary VARCHAR NOT NULL
);

SELECT diesel_manage_updated_at('strange_blockchain_transactions');
