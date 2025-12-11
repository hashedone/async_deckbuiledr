//! Users related API tests

use actix_web::{App, test};
use serde_json::json;

use crate::service;
use crate::service::tests::{GraphQLResp, gql};

#[actix_web::test]
async fn create_ad_hoc_users() {
    let app = App::new().configure(service::configure(false));
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
            .set_payload(query)
            .to_request(),
    )
    .await;

    resp.data::<String>("users.createAdhoc.token");
    assert_eq!(
        resp.data::<String>("users.createAdhoc.user.nickname"),
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
            .set_payload(query)
            .to_request(),
    )
    .await;

    assert_eq!(resp.data::<String>("users.m1.user.nickname"), "user2");
    assert_eq!(resp.data::<String>("users.m2.user.nickname"), "user1");
}
