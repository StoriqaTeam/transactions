CREATE INDEX seen_hashes_hash_idx ON seen_hashes (hash);
CREATE INDEX accounts_address_idx ON accounts (address);
CREATE INDEX blockchain_transactions_hash_idx ON blockchain_transactions (hash);
CREATE INDEX transactions_dr_account_id_idx ON transactions (dr_account_id);
CREATE INDEX transactions_cr_account_id_idx ON transactions (cr_account_id);
