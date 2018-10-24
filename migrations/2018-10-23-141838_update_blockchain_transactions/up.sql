ALTER TABLE blockchain_transactions DROP COLUMN from_;
ALTER TABLE blockchain_transactions ADD COLUMN from_ JSONB NOT NULL DEFAULT '[]';
ALTER TABLE blockchain_transactions DROP COLUMN to_;
ALTER TABLE blockchain_transactions ADD COLUMN to_ JSONB NOT NULL DEFAULT '[]';
