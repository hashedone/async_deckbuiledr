//! Lobby related API tests

use actix_web::{App, test};
use serde_json::json;

use crate::model::Model;
use crate::model::users::UserId;
use crate::service;
use crate::service::tests::gql;

#[actix_web::test]
async fn create_two_lobby_games() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                        user
                    }
                }
            }"#)
    .variables(json!({ "name": "user1" }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let adhoc_token: String = resp.data("users.createAdhoc.token").unwrap();
    let user_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp = gql(r#"mutation {
            lobby {
                createGame
            }
        }"#)
    .adhoc(&adhoc_token)
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let game_id1: String = resp.data("lobby.createGame").unwrap();

    let resp = gql(r#"mutation {
            lobby {
                createGame
            }
        }"#)
    .adhoc(&adhoc_token)
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let game_id2: String = resp.data("lobby.createGame").unwrap();

    assert_ne!(game_id1, game_id2);

    let resp = gql(r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#)
    .variables(json!({ "id": game_id1 }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(user_id, created_by);
    assert_eq!(players, vec![]);

    let resp = gql(r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#)
    .variables(json!({ "id": game_id2 }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(user_id, created_by);
    assert_eq!(players, vec![]);
}

#[actix_web::test]
async fn lobby_game_join_flow() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                        user
                    }
                }
            }"#)
    .variables(json!({ "name": "creator" }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let creator_token: String = resp.data("users.createAdhoc.token").unwrap();
    let creator_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                        user
                    }
                }
            }"#)
    .variables(json!({ "name": "player1" }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let player1_token: String = resp.data("users.createAdhoc.token").unwrap();
    let player1_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                        user
                    }
                }
            }"#)
    .variables(json!({ "name": "player2" }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let player2_token: String = resp.data("users.createAdhoc.token").unwrap();
    let player2_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp = gql(r#"mutation {
            lobby {
                createGame
            }
        }"#)
    .adhoc(&creator_token)
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let game_id: String = resp.data("lobby.createGame").unwrap();

    let resp = gql(r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#)
    .variables(json!({ "id": game_id }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(creator_id, created_by);
    assert_eq!(players, vec![]);

    let resp = gql(r#"mutation($id: GameId!) {
            lobby {
                joinGame(gameId: $id)
            }
        }"#)
    .variables(json!({ "id": game_id }))
    .adhoc(&player1_token)
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let joined_game_id: String = resp.data("lobby.joinGame").unwrap();
    assert_eq!(joined_game_id, game_id);

    let resp = gql(r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#)
    .variables(json!({ "id": game_id }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(creator_id, created_by);
    assert_eq!(players, vec![player1_id]);

    let resp = gql(r#"mutation($id: GameId!) {
            lobby {
                joinGame(gameId: $id)
            }
        }"#)
    .variables(json!({ "id": game_id }))
    .adhoc(&player2_token)
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let joined_game_id: String = resp.data("lobby.joinGame").unwrap();
    assert_eq!(joined_game_id, game_id);

    let resp = gql(r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#)
    .variables(json!({ "id": game_id }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(creator_id, created_by);
    assert_eq!(players, vec![player1_id, player2_id]);
}
