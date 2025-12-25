-- Info about games being created
create table lobby (
    -- Created game ID
    id blob primary key not null,
    -- User that created the game
    created_by integer references users(id) not null,
    -- Player IDs - can be null as the game didn't yet start
    player1 integer references users(id),
    player2 integer references users(id)
);
