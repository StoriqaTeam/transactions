ALTER TABLE tx_groups
  DROP COLUMN user_id,
  DROP COLUMN blockchain_tx_id;

ALTER TABLE transactions
  ADD COLUMN user_id UUID NOT NULL DEFAULT uuid_generate_v4() REFERENCES users,
  ADD COLUMN blockchain_tx_id VARCHAR;

CREATE INDEX tx_groups_blockchain_tx_id ON tx_groups (blockchain_tx_id);

