//! Users related API tests

use actix_web::{App, test};
use serde_json::json;

use crate::model::Model;
use crate::model::users::UserId;
use crate::service;
use crate::service::tests::{GraphQLResp, gql};

#[actix_web::test]
async fn create_ad_hoc_users() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    // Adding single user
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

    println!("{resp:?}");
    resp.data::<String>("users.createAdhoc.token").unwrap();
    let user1: UserId = resp.data("users.createAdhoc.user").unwrap();

    let query = gql(
        r#"query($id: UserId!) { user(id: $id) { nickname } }"#,
        json!({ "id": user1 }),
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

    assert_eq!(None, resp.errors);
    let nickname: String = resp.data("user.nickname").unwrap();
    assert_eq!("user1", nickname);

    // Add two more users, introduce nickname collistion
    let query = gql(
        r#"mutation($name1: String!, $name2: String!) {
            users {
                m1: createAdhoc(nickname: $name1) { user }
                m2: createAdhoc(nickname: $name2) { user }
            }
        }"#,
        json!({ "name1": "user2", "name2": "user1" }),
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

    // Verify all users are distinct
    let user2: UserId = resp.data("users.m1.user").unwrap();
    let user3: UserId = resp.data("users.m2.user").unwrap();

    let query = gql(
        r#"query($id: UserId!) { user(id: $id) { nickname } }"#,
        json!({ "id": user2 }),
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

    assert_eq!(None, resp.errors);
    let nickname: String = resp.data("user.nickname").unwrap();
    assert_eq!("user2", nickname);

    let query = gql(
        r#"query($id: UserId!) { user(id: $id) { nickname } }"#,
        json!({ "id": user3 }),
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

    assert_eq!(None, resp.errors);
    let nickname: String = resp.data("user.nickname").unwrap();
    assert_eq!("user1", nickname);

    assert_ne!(user1, user2);
    assert_ne!(user2, user3);
    assert_ne!(user1, user3);
}

#[actix_web::test]
async fn refresh_without_session_header() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let resp =
        test::call_service(&app, test::TestRequest::get().uri("/refresh").to_request()).await;

    assert!(resp.status().is_success());
    assert!(!resp.headers().contains_key("x-session-token"));
}

#[actix_web::test]
async fn refresh_with_adhoc_session_flow() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let query = gql(
        r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
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

    assert_eq!(None, resp.errors);
    let adhoc_token = resp.data::<String>("users.createAdhoc.token").unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("AdHoc {adhoc_token}")))
            .to_request(),
    )
    .await;

    assert!(resp.status().is_success());
    let session_token1 = resp
        .headers()
        .get("x-session-token")
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .to_string();

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("AdHoc {adhoc_token}")))
            .to_request(),
    )
    .await;

    assert!(resp.status().is_success());
    let session_token2 = resp
        .headers()
        .get("x-session-token")
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .to_string();

    assert_ne!(session_token1, session_token2);

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("Session {session_token1}")))
            .to_request(),
    )
    .await;

    assert!(resp.status().is_success());
    assert!(!resp.headers().contains_key("x-session-token"));
}

#[actix_web::test]
async fn refresh_with_invalid_authorization_header() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let err = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", "junk"))
            .to_request(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.as_response_error().status_code(),
        actix_web::http::StatusCode::UNAUTHORIZED
    );

    let err = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", "AdHoc junk"))
            .to_request(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.as_response_error().status_code(),
        actix_web::http::StatusCode::UNAUTHORIZED
    );

    let err = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", "Session junk"))
            .to_request(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.as_response_error().status_code(),
        actix_web::http::StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn expire_session_without_authorization() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let resp = test::call_service(
        &app,
        test::TestRequest::delete().uri("/session").to_request(),
    )
    .await;

    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn expire_session_rejects_deleted_token() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let query = gql(
        r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
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

    assert_eq!(None, resp.errors);
    let adhoc_token = resp.data::<String>("users.createAdhoc.token").unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("AdHoc {adhoc_token}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());

    let session_token = resp
        .headers()
        .get("x-session-token")
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .to_string();

    let resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/session")
            .insert_header(("Authorization", format!("Session {session_token}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());

    let err = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("Session {session_token}")))
            .to_request(),
    )
    .await
    .expect_err("expected unauthorized error");
    assert_eq!(
        err.as_response_error().status_code(),
        actix_web::http::StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn adhoc_session_refresh_and_expire_flow() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    let query = gql(
        r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
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

    let adhoc_token = resp.data::<String>("users.createAdhoc.token").unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("AdHoc {adhoc_token}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());

    let session_token = resp
        .headers()
        .get("x-session-token")
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .to_string();

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("Session {session_token}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    assert!(!resp.headers().contains_key("x-session-token"));

    let resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/session")
            .insert_header(("Authorization", format!("Session {session_token}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());

    let err = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/refresh")
            .insert_header(("Authorization", format!("Session {session_token}")))
            .to_request(),
    )
    .await
    .unwrap_err();

    assert_eq!(
        err.as_response_error().status_code(),
        actix_web::http::StatusCode::UNAUTHORIZED
    );
}
