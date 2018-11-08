CREATE TABLE tx_groups (
    id UUID PRIMARY KEY,
    kind VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    tx_1 UUID,
    tx_2 UUID,
    tx_3 UUID,
    tx_4 UUID,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('tx_groups');
