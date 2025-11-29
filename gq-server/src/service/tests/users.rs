//! Users related API tests

use std::str::from_utf8;

use assert_json_diff::assert_json_eq;
use serde_json::{Value, json};

use crate::service::tests::{gql, setup};

#[tokio::test]
async fn users_empty_initially() {
    let service = setup();

    let res = gql("query { users { all { user { nickname } } } }", json!(null))
        .reply(&service)
        .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    let users: Value = serde_json::from_slice(res.body()).unwrap();
    assert_json_eq!(
        json!({
            "data": {
                "users": {
                    "all": []
                }
            }
        }),
        users
    );
}

#[tokio::test]
async fn illformed_user_id() {
    let service = setup();

    let res = gql(
        r#"query($id: String!) { users { id(userId: $id) { user { nickname } } } }"#,
        json!({ "id": "illformed" }),
    )
    .reply(&service)
    .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    let user: Value = serde_json::from_slice(res.body()).unwrap();
    assert_json_eq!(json!(null), user["data"]);
    assert_eq!(1, user["errors"].as_array().unwrap().len());
}

#[tokio::test]
async fn not_existing_user_id() {
    let service = setup();

    let res = gql(
        r#"query($id: String!) { users { id(userId: $id) { user { nickname } } } }"#,
        json!({ "id": "PpPH9UytRv2nJ5Wy4VpXHA==" }),
    )
    .reply(&service)
    .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    let user: Value = serde_json::from_slice(res.body()).unwrap();
    assert_json_eq!(
        json!({
            "data": {
                "users": {
                    "id": {
                        "user": null
                    }
                }
            }
        }),
        user
    );
}

#[tokio::test]
async fn create_ad_hoc_users() {
    let service = setup();

    // Adding single user
    let res = gql(
        r#"mutation($name: String!) {
            users {
                createAdhoc(nickname: $name) {
                    user { user { nickname } }
                }
            }
        }"#,
        json!({ "name": "user1" }),
    )
    .reply(&service)
    .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    let user: Value = serde_json::from_slice(res.body()).unwrap();
    let user = user["data"]["users"]["createAdhoc"].clone();
    assert_json_eq!(
        json!({
            "user": {
                "user": {
                    "nickname": "user1"
                }
            }
        }),
        user
    );

    // Check if the user is visible
    let res = gql("query { users { all { user { nickname } } } }", json!(null))
        .reply(&service)
        .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    let users: Value = serde_json::from_slice(res.body()).unwrap();
    assert_json_eq!(
        json!({
            "data": {
                "users": {
                    "all": [
                        { "user": { "nickname": "user1" } }
                    ]
                }
            }
        }),
        users
    );

    // Add two more user, introduce nickname collistion
    let res = gql(
        r#"mutation($name1: String!, $name2: String!) {
            users {
                m1: createAdhoc(nickname: $name1) { token }
                m2: createAdhoc(nickname: $name2) { token }
            }
        }"#,
        json!({ "name1": "user2", "name2": "user1" }),
    )
    .reply(&service)
    .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    // Check if all the users are visible
    let res = gql(
        r#"query {
            users {
                all {
                    user {
                        nickname
                    }
                }
            }
        }"#,
        json!(null),
    )
    .reply(&service)
    .await;
    assert_eq!(res.status(), 200, "{}", from_utf8(res.body()).unwrap());

    let users: Value = serde_json::from_slice(res.body()).unwrap();
    let mut users = users["data"]["users"]["all"].clone();
    users
        .as_array_mut()
        .unwrap()
        .sort_by_key(|user| user["user"]["nickname"].as_str().unwrap().to_owned());

    assert_json_eq!(
        json!([
                { "user": { "nickname": "user1" } },
                { "user": { "nickname": "user1" } },
                { "user": { "nickname": "user2" } }
            ]
        ),
        users
    );
}
