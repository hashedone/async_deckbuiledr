//! Services integration tests

use actix_http::Request;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::test;
use color_eyre::eyre::eyre;
use color_eyre::{Result, eyre::bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, from_value, json};

mod lobby;
mod users;

/// Builder for GraphQL test requests
#[derive(Debug, Clone)]
struct GraphQLTestRequest {
    /// GQL query
    query: String,
    /// Variables to attach
    variables: Value,
    /// Authorization header
    authorization: Option<String>,
}

impl GraphQLTestRequest {
    /// Creates new GraphQL test request
    fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            variables: json!({}),
            authorization: None,
        }
    }

    /// Attaches variables
    fn variables(self, variables: Value) -> Self {
        Self { variables, ..self }
    }

    /// Attaches ad-hoc authorization header
    fn adhoc(self, token: &str) -> Self {
        Self {
            authorization: Some(format!("AdHoc {token}")),
            ..self
        }
    }

    /// Calls the request
    async fn call<S, B>(self, app: &S) -> Result<GraphQLResp>
    where
        S: Service<Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
        B: MessageBody,
    {
        let query: String = self.query.lines().collect();
        let payload = format!(
            r#"{{ "query": "{query}", "variables": {variables} }}"#,
            variables = self.variables
        );
        let mut req = test::TestRequest::post()
            .uri("/api")
            .insert_header(("content-type", "application/json"));

        if let Some(header) = self.authorization {
            req = req.insert_header(("Authorization", header));
        }

        let req = req.set_payload(payload).to_request();
        test::try_call_and_read_body_json(app, req)
            .await
            .map_err(|err| eyre!("{err:?}"))
    }
}

fn gql(query: &str) -> GraphQLTestRequest {
    GraphQLTestRequest::new(query)
}

#[derive(Debug, Deserialize, Clone)]
struct GraphQLResp {
    /// GraphQL data.
    ///
    /// It can technically be empty, but we assume for testing purposes that data are returned - if
    /// the are not there, that means an error occured and the deserialization will fail.
    ///
    /// It is still possible to make this optional passing `T` as `Option<_>` for testing the
    /// error path.
    pub data: Value,
    /// GraphQL errors
    pub errors: Option<Vec<serde_json::Value>>,
}

impl GraphQLResp {
    /// Returns the deserialized data part at given json path
    fn data<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let data = path.split('.').fold(Ok(&self.data), |data, key| {
            let data = data?;
            let Value::Object(fields) = data else {
                bail!("{path} is not a valid path in {:?}", self.data);
            };

            let Some(data) = fields.get(key) else {
                bail!("{path} is not a valid path in {:?}", self.data);
            };

            Ok(data)
        })?;

        from_value(data.clone()).map_err(Into::into)
    }
}
