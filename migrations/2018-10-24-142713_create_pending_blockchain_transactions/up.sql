CREATE TABLE pending_blockchain_transactions (
    hash VARCHAR PRIMARY KEY,
    from_ VARCHAR NOT NULL,
    to_ VARCHAR NOT NULL,
    currency VARCHAR NOT NULL,
    value NUMERIC NOT NULL,
    fee NUMERIC NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('pending_blockchain_transactions');
