ALTER TABLE transactions
  DROP COLUMN user_id,
  DROP COLUMN blockchain_tx_id;

ALTER TABLE tx_groups
  ADD COLUMN user_id UUID NOT NULL REFERENCES users,
  ADD COLUMN blockchain_tx_id VARCHAR;

CREATE INDEX tx_groups_blockchain_tx_id ON tx_groups (blockchain_tx_id);

