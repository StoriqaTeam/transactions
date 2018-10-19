-- Your SQL goes here
CREATE TABLE blockchain_transactions (
    hash VARCHAR PRIMARY KEY,
    from_ VARCHAR NOT NULL,
    to_ VARCHAR NOT NULL,
    block_number BIGINT NOT NULL,
    currency VARCHAR NOT NULL,
    value NUMERIC NOT NULL,
    fee NUMERIC NOT NULL,
    confirmations INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('blockchain_transactions');
