ALTER TABLE blockchain_transactions
  ADD COLUMN erc20_operation_kind VARCHAR;

ALTER TABLE pending_blockchain_transactions
  ADD COLUMN erc20_operation_kind VARCHAR;

ALTER TABLE strange_blockchain_transactions
  ADD COLUMN erc20_operation_kind VARCHAR;
