-- Your SQL goes here
CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users,
    dr_account_id UUID NOT NULL REFERENCES accounts,
    cr_account_id UUID NOT NULL REFERENCES accounts,
    currency VARCHAR NOT NULL,
    value NUMERIC NOT NULL,
    status VARCHAR NOT NULL,
    blockchain_tx_id VARCHAR,
    hold_until TIMESTAMP DEFAULT current_timestamp,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('transactions');
