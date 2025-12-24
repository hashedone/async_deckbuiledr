//! Users related API tests

use actix_web::{App, test};
use serde_json::json;

use crate::model::Model;
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
                        user { nickname }
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
    assert_eq!(
        resp.data::<String>("users.createAdhoc.user.nickname")
            .unwrap(),
        "user1"
    );

    // Add two more user, introduce nickname collistion
    let query = gql(
        r#"mutation($name1: String!, $name2: String!) {
            users {
                m1: createAdhoc(nickname: $name1) { user { nickname } }
                m2: createAdhoc(nickname: $name2) { user { nickname } }
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

    assert_eq!(
        resp.data::<String>("users.m1.user.nickname").unwrap(),
        "user2"
    );
    assert_eq!(
        resp.data::<String>("users.m2.user.nickname").unwrap(),
        "user1"
    );
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
