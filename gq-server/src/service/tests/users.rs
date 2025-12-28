//! Users related API tests

use actix_web::{App, test};
use serde_json::json;

use crate::model::Model;
use crate::model::users::UserId;
use crate::service;
use crate::service::tests::gql;

#[actix_web::test]
async fn create_ad_hoc_users() {
    let context = Model::test().await.unwrap();
    let service_config = service::configure(false, context).await.unwrap();
    let app = App::new().configure(service_config);
    let app = test::init_service(app).await;

    // Adding single user
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

    println!("{resp:?}");
    resp.data::<String>("users.createAdhoc.token").unwrap();
    let user1: UserId = resp.data("users.createAdhoc.user").unwrap();

    let resp = gql(r#"query($id: UserId!) { user(id: $id) { nickname } }"#)
        .variables(json!({ "id": user1 }))
        .call(&app)
        .await
        .unwrap();

    assert_eq!(None, resp.errors);
    let nickname: String = resp.data("user.nickname").unwrap();
    assert_eq!("user1", nickname);

    // Add two more users, introduce nickname collistion
    let resp = gql(r#"mutation($name1: String!, $name2: String!) {
            users {
                m1: createAdhoc(nickname: $name1) { user }
                m2: createAdhoc(nickname: $name2) { user }
            }
        }"#)
    .variables(json!({ "name1": "user2", "name2": "user1" }))
    .call(&app)
    .await
    .unwrap();

    // Verify all users are distinct
    let user2: UserId = resp.data("users.m1.user").unwrap();
    let user3: UserId = resp.data("users.m2.user").unwrap();

    let resp = gql(r#"query($id1: UserId!, $id2: UserId!) {
            u1: user(id: $id1) { nickname },
            u2: user(id: $id2) { nickname }
        }"#)
    .variables(json!({ "id1": user2, "id2": user3 }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(None, resp.errors);

    let nickname: String = resp.data("u1.nickname").unwrap();
    assert_eq!("user2", nickname);

    let nickname: String = resp.data("u2.nickname").unwrap();
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

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                    }
                }
            }"#)
    .variables(json!({ "name": "user1" }))
    .call(&app)
    .await
    .unwrap();

    assert_eq!(None, resp.errors);
    let adhoc_token: String = resp.data("users.createAdhoc.token").unwrap();

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

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                    }
                }
            }"#)
    .variables(json!({ "name": "user1" }))
    .call(&app)
    .await
    .unwrap();

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

    let resp = gql(r#"mutation($name: String!) {
                users {
                    createAdhoc(nickname: $name) {
                        token
                    }
                }
            }"#)
    .variables(json!({ "name": "user1" }))
    .call(&app)
    .await
    .unwrap();

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
