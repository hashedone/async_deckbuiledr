-- Ad-hoc created user tokens
create table user_tokens(
  -- Token id
  id blob primary key not null,
  -- User id authenticated by this token
  user_id int not null,
  -- Secret to build the signing key
  secret blob not null,
  -- Expected signature for this token. Signature is expected to be unique,
  -- if not the secret should be regenerated to get unique signature.
  signature blob unique not null
) ;

-- Session Paseto tokens
create table session_tokens(
  -- Token id
  id blob primary key not null,
  -- Public key for token verification
  public_key text not null
) ;
