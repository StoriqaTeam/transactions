ALTER TABLE tx_groups
  DROP COLUMN user_id,
  DROP COLUMN blockchain_tx_id;

ALTER TABLE transactions
  ADD COLUMN user_id UUID NOT NULL REFERENCES users,
  ADD COLUMN blockchain_tx_id VARCHAR;

DROP INDEX IF EXISTS tx_groups_blockchain_tx_id_idx;

