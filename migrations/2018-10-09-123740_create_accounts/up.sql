-- Your SQL goes here
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users,
    currency VARCHAR NOT NULL,
    address VARCHAR NOT NULL,
    name VARCHAR,
    kind VARCHAR NOT NULL,
    balance NUMERIC NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('accounts');
