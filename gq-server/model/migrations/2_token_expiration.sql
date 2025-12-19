-- Adds the `expires_at` column to the `session_tokens` table holding information when the token
-- expires. For the entries already in the table, expiration should be set to 1 hour from now.
-- New entries must supply the expiration explicitly.
ALTER TABLE session_tokens ADD COLUMN expires_at TIMESTAMP;

UPDATE session_tokens
SET expires_at = datetime('now', '+1 hour')
WHERE expires_at IS NULL;

CREATE TABLE session_tokens_new(
  -- Token id
  id blob primary key not null,
  -- Public key for token verification
  public_key text not null,
  -- Expiration timestamp
  expires_at timestamp not null
);

INSERT INTO session_tokens_new(id, public_key, expires_at)
SELECT id, public_key, expires_at
FROM session_tokens;

DROP TABLE session_tokens;
ALTER TABLE session_tokens_new RENAME TO session_tokens;
