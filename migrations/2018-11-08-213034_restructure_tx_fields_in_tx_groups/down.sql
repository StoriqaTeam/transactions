ALTER TABLE tx_groups
  ADD COLUMN tx_1 UUID,
  ADD COLUMN tx_2 UUID,
  ADD COLUMN tx_3 UUID,
  ADD COLUMN tx_4 UUID,
  DROP COLUMN base_tx,
  DROP COLUMN from_tx,
  DROP COLUMN to_tx,
  DROP COLUMN fee_tx,
  DROP COLUMN withdrawal_txs;
