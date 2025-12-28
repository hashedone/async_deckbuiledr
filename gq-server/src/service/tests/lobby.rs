//! Lobby related API tests

use actix_web::{App, test};
use serde_json::json;

use crate::model::Model;
use crate::model::users::UserId;
use crate::service;
use crate::service::tests::{GraphQLResp, gql};

#[actix_web::test]
async fn create_two_lobby_games() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let query = gql(
        r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                        user
                    }
                }
            }"#,
        json!({ "name": "user1" }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let adhoc_token = resp.data::<String>("users.createAdhoc.token").unwrap();
    let user_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let query = gql(
        r#"mutation {
            lobby {
                createGame
            }
        }"#,
        json!({}),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .insert_header(("Authorization", format!("AdHoc {adhoc_token}")))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let game_id1 = resp.data::<String>("lobby.createGame").unwrap();

    let query = gql(
        r#"mutation {
            lobby {
                createGame
            }
        }"#,
        json!({}),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .insert_header(("Authorization", format!("AdHoc {adhoc_token}")))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let game_id2 = resp.data::<String>("lobby.createGame").unwrap();

    assert_ne!(game_id1, game_id2);

    let query = gql(
        r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#,
        json!({ "id": game_id1 }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(user_id, created_by);
    assert_eq!(players, vec![]);

    let query = gql(
        r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#,
        json!({ "id": game_id2 }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(query)
            .to_request(),
    )
    .await;

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

    let create_user = |name: &str| {
        gql(
            r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                        user
                    }
                }
            }"#,
            json!({ "name": name }),
        )
    };

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(create_user("creator"))
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let creator_token: String = resp.data("users.createAdhoc.token").unwrap();
    let creator_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(create_user("player1"))
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let player1_token: String = resp.data("users.createAdhoc.token").unwrap();
    let player1_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(create_user("player2"))
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let player2_token: String = resp.data("users.createAdhoc.token").unwrap();
    let player2_id: UserId = resp.data("users.createAdhoc.user").unwrap();

    let query = gql(
        r#"mutation {
            lobby {
                createGame
            }
        }"#,
        json!({}),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .insert_header(("Authorization", format!("AdHoc {creator_token}")))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let game_id: String = resp.data("lobby.createGame").unwrap();

    let query = gql(
        r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#,
        json!({ "id": game_id }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(creator_id, created_by);
    assert_eq!(players, vec![]);

    let query = gql(
        r#"mutation($id: GameId!) {
            lobby {
                joinGame(gameId: $id)
            }
        }"#,
        json!({ "id": game_id }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .insert_header(("Authorization", format!("AdHoc {player1_token}")))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let joined_game_id: String = resp.data("lobby.joinGame").unwrap();
    assert_eq!(joined_game_id, game_id);

    let query = gql(
        r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#,
        json!({ "id": game_id }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(creator_id, created_by);
    assert_eq!(players, vec![player1_id]);

    let query = gql(
        r#"mutation($id: GameId!) {
            lobby {
                joinGame(gameId: $id)
            }
        }"#,
        json!({ "id": game_id }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .insert_header(("Authorization", format!("AdHoc {player2_token}")))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let joined_game_id: String = resp.data("lobby.joinGame").unwrap();
    assert_eq!(joined_game_id, game_id);

    let query = gql(
        r#"query($id: GameId!) {
            lobby(id: $id) {
                createdBy
                players
            }
        }"#,
        json!({ "id": game_id }),
    );

    let resp: GraphQLResp = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"))
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.errors, None);
    let created_by: UserId = resp.data("lobby.createdBy").unwrap();
    let players: Vec<UserId> = resp.data("lobby.players").unwrap();
    assert_eq!(creator_id, created_by);
    assert_eq!(players, vec![player1_id, player2_id]);
}
