ALTER TABLE accounts
  ADD COLUMN daily_limit_type VARCHAR NOT NULL DEFAULT 'default';
