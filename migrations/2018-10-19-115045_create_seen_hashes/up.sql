-- Your SQL goes here
CREATE TABLE seen_hashes (
    hash VARCHAR PRIMARY KEY,
    block_number BIGINT NOT NULL,
    currency VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('seen_hashes');
