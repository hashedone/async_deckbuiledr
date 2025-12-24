//! Services integration tests

use color_eyre::{Result, eyre::bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, from_value};

mod users;

/// Prepares graphql API request body
fn gql(query: &str, variables: Value) -> String {
    let query: String = query.lines().collect();
    format!(r#"{{ "query": "{query}", "variables": {variables} }}"#)
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
