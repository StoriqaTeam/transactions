ALTER TABLE seen_hashes DROP CONSTRAINT seen_hashes_pkey;
ALTER TABLE seen_hashes ADD PRIMARY KEY hash;
