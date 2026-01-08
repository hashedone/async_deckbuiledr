-- Ongoing games table
create table games (
    -- Created game ID
    id blob primary key not null,
    -- User that created the game
    created_by integer references users(id) not null,
    -- Player IDs - game already started so both are required
    player1 blob references users(id) not null,
    player2 blob references users(id) not null
);
