-- Convert user ids to UUID and drop existing data referencing them.
PRAGMA foreign_keys = OFF;

DELETE FROM lobby;
DELETE FROM adhoc_tokens;
DELETE FROM users;

CREATE TABLE users_new(
  -- User id
  id blob primary key not null,
  -- Nickname used by an user. Nicknames *can* collide.
  nickname text
);

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE TABLE adhoc_tokens_new(
  -- Token id
  id blob primary key not null,
  -- User id authenticated by this token
  user_id blob not null,
  -- Secret to build the signing key
  secret blob not null,
  -- Expected signature for this token. Signature is expected to be unique,
  -- if not the secret should be regenerated to get unique signature.
  signature blob unique not null
);

DROP TABLE adhoc_tokens;
ALTER TABLE adhoc_tokens_new RENAME TO adhoc_tokens;

CREATE TABLE lobby_new (
  -- Created game ID
  id blob primary key not null,
  -- User that created the game
  created_by blob references users(id) not null,
  -- Player IDs - can be null as the game didn't yet start
  player1 blob references users(id),
  player2 blob references users(id)
);

DROP TABLE lobby;
ALTER TABLE lobby_new RENAME TO lobby;

PRAGMA foreign_keys = ON;
PRAGMA foreign_key_check;
