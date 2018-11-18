ALTER TABLE blockchain_transactions
  DROP COLUMN erc20_operation_kind;

ALTER TABLE pending_blockchain_transactions
  DROP COLUMN erc20_operation_kind;

ALTER TABLE strange_blockchain_transactions
  DROP COLUMN erc20_operation_kind;
