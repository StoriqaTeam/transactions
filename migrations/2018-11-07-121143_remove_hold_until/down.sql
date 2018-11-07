ALTER TABLE transactions ADD COLUMN hold_until TIMESTAMP DEFAULT current_timestamp;
