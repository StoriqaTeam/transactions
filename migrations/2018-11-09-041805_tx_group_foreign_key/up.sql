ALTER TABLE transactions
  ADD CONSTRAINT transactions_gid_fk FOREIGN KEY (gid) REFERENCES tx_groups(id);
