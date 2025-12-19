-- Adds the `expires_at` column to the `session_tokens` table holding information when the token
-- expires. For the entries already in the table, expiration should be set to 1 hour from now.
ALTER TABLE session_tokens ADD COLUMN expires_at TIMESTAMP NOT NULL DEFAULT NOW() + INTERVAL '1 hour';
