-- Basic users table
create table users(
  -- User id
  id integer primary key autoincrement not null,
  -- Nickname used by an user. Nicknames *can* collide.
  nickname text
)
