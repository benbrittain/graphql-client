//! The top-level documentation resides on the [project
//! README](https://github.com/graphql-rust/graphql-client) at the moment.
//!
//! The main interface to this library is the custom derive that generates
//! modules from a GraphQL query and schema. See the docs for the
//! [`GraphQLQuery`] trait for a full example.
//!
//! ## Cargo features
//!
//! - `graphql_query_derive` (default: on): enables the `#[derive(GraphqlQuery)]` custom derive.
//! - `reqwest` (default: off): exposes the `graphql_client::reqwest::post_graphql()` function.
//! - `reqwest-blocking` (default: off): exposes the blocking version, `graphql_client::reqwest::post_graphql_blocking()`.

#![deny(missing_docs)]
#![warn(rust_2018_idioms)]

#[cfg(feature = "graphql_query_derive")]
#[allow(unused_imports)]
#[macro_use]
extern crate graphql_query_derive;

#[cfg(feature = "graphql_query_derive")]
#[doc(hidden)]
pub use graphql_query_derive::*;

#[cfg(any(
    feature = "reqwest",
    feature = "reqwest-rustls",
    feature = "reqwest-blocking"
))]
pub mod reqwest;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Write};

/// A convenience trait that can be used to build a GraphQL request body.
///
/// This will be implemented for you by codegen in the normal case. It is implemented on the struct you place the derive on.
///
/// Example:
///
/// ```
/// use graphql_client::*;
/// use serde_json::json;
/// use std::error::Error;
///
/// #[derive(GraphQLQuery)]
/// #[graphql(
///   query_path = "../graphql_client_codegen/src/tests/star_wars_query.graphql",
///   schema_path = "../graphql_client_codegen/src/tests/star_wars_schema.graphql"
/// )]
/// struct StarWarsQuery;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     use graphql_client::GraphQLQuery;
///
///     let variables = star_wars_query::Variables {
///         episode_for_hero: star_wars_query::Episode::NEWHOPE,
///     };
///
///     let expected_body = json!({
///         "operationName": star_wars_query::OPERATION_NAME,
///         "query": star_wars_query::QUERY,
///         "variables": {
///             "episodeForHero": "NEWHOPE"
///         },
///     });
///
///     let actual_body = serde_json::to_value(
///         StarWarsQuery::build_query(variables)
///     )?;
///
///     assert_eq!(actual_body, expected_body);
///
///     Ok(())
/// }
/// ```
pub trait GraphQLQuery {
    /// The shape of the variables expected by the query. This should be a generated struct most of the time.
    type Variables: serde::Serialize;
    /// The top-level shape of the response data (the `data` field in the GraphQL response). In practice this should be generated, since it is hard to write by hand without error.
    type ResponseData: for<'de> serde::Deserialize<'de>;

    /// Produce a GraphQL query struct that can be JSON serialized and sent to a GraphQL API.
    fn build_query(variables: Self::Variables) -> QueryBody<Self::Variables>;

    /// Produce a persisted query body using the pre-computed SHA256 hash.
    ///
    /// This is for static persisted queries where the server has the query
    /// pre-registered and only needs the hash to identify it. The query string
    /// is not included in the request body.
    fn build_persisted_query(variables: Self::Variables) -> PersistedQueryBody<Self::Variables>;
}

/// The form in which queries are sent over HTTP in most implementations. This will be built using the [`GraphQLQuery`] trait normally.
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryBody<Variables> {
    /// The values for the variables. They must match those declared in the queries. This should be the `Variables` struct from the generated module corresponding to the query.
    pub variables: Variables,
    /// The GraphQL query, as a string.
    pub query: &'static str,
    /// The GraphQL operation name, as a string.
    #[serde(rename = "operationName")]
    pub operation_name: &'static str,
}

/// The persisted query extension format used by Apollo.
///
/// This is included in the `extensions` field of a [`PersistedQueryBody`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PersistedQueryExtension {
    /// The version of the persisted query protocol (always 1).
    pub version: u8,
    /// The SHA256 hash of the query string.
    #[serde(rename = "sha256Hash")]
    pub sha256_hash: &'static str,
}

/// Extensions object containing persisted query information.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PersistedQueryExtensions {
    /// The persisted query extension.
    #[serde(rename = "persistedQuery")]
    pub persisted_query: PersistedQueryExtension,
}

/// Request body for static persisted queries (Apollo format).
///
/// Unlike [`QueryBody`], this omits the `query` field entirely since
/// the server already has the query registered. Only the SHA256 hash
/// is sent to identify the query.
///
/// Example JSON output:
/// ```json
/// {
///   "operationName": "MyQuery",
///   "variables": {},
///   "extensions": {
///     "persistedQuery": {
///       "version": 1,
///       "sha256Hash": "abc123..."
///     }
///   }
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct PersistedQueryBody<Variables> {
    /// The values for the variables.
    pub variables: Variables,
    /// The GraphQL operation name, as a string.
    #[serde(rename = "operationName")]
    pub operation_name: &'static str,
    /// Extensions containing the persisted query hash.
    pub extensions: PersistedQueryExtensions,
}

impl<Variables> PersistedQueryBody<Variables> {
    /// Create a new persisted query body.
    pub fn new(
        variables: Variables,
        operation_name: &'static str,
        query_hash: &'static str,
    ) -> Self {
        Self {
            variables,
            operation_name,
            extensions: PersistedQueryExtensions {
                persisted_query: PersistedQueryExtension {
                    version: 1,
                    sha256_hash: query_hash,
                },
            },
        }
    }
}

/// Represents a location inside a query string. Used in errors. See [`Error`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Location {
    /// The line number in the query string where the error originated (starting from 1).
    pub line: i32,
    /// The column number in the query string where the error originated (starting from 1).
    pub column: i32,
}

/// Part of a path in a query. It can be an object key or an array index. See [`Error`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PathFragment {
    /// A key inside an object
    Key(String),
    /// An index inside an array
    Index(i32),
}

impl Display for PathFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PathFragment::Key(ref key) => write!(f, "{}", key),
            PathFragment::Index(ref idx) => write!(f, "{}", idx),
        }
    }
}

/// An element in the top-level `errors` array of a response body.
///
/// This tries to be as close to the spec as possible.
///
/// [Spec](https://github.com/facebook/graphql/blob/master/spec/Section%207%20--%20Response.md)
///
///
/// ```
/// # use serde_json::json;
/// # use serde::Deserialize;
/// # use graphql_client::GraphQLQuery;
/// # use std::error::Error;
/// #
/// # #[derive(Debug, Deserialize, PartialEq)]
/// # struct ResponseData {
/// #     something: i32
/// # }
/// #
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use graphql_client::*;
///
/// let body: Response<ResponseData> = serde_json::from_value(json!({
///     "data": null,
///     "errors": [
///         {
///             "message": "The server crashed. Sorry.",
///             "locations": [{ "line": 1, "column": 1 }]
///         },
///         {
///             "message": "Seismic activity detected",
///             "path": ["underground", 20]
///         },
///      ],
/// }))?;
///
/// let expected: Response<ResponseData> = Response {
///     data: None,
///     errors: Some(vec![
///         Error {
///             message: "The server crashed. Sorry.".to_owned(),
///             locations: Some(vec![
///                 Location {
///                     line: 1,
///                     column: 1,
///                 }
///             ]),
///             path: None,
///             extensions: None,
///         },
///         Error {
///             message: "Seismic activity detected".to_owned(),
///             locations: None,
///             path: Some(vec![
///                 PathFragment::Key("underground".into()),
///                 PathFragment::Index(20),
///             ]),
///             extensions: None,
///         },
///     ]),
///     extensions: None,
/// };
///
/// assert_eq!(body, expected);
///
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Error {
    /// The human-readable error message. This is the only required field.
    pub message: String,
    /// Which locations in the query the error applies to.
    pub locations: Option<Vec<Location>>,
    /// Which path in the query the error applies to, e.g. `["users", 0, "email"]`.
    pub path: Option<Vec<PathFragment>>,
    /// Additional errors. Their exact format is defined by the server.
    pub extensions: Option<HashMap<String, serde_json::Value>>,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use `/` as a separator like JSON Pointer.
        let path = self
            .path
            .as_ref()
            .map(|fragments| {
                fragments
                    .iter()
                    .fold(String::new(), |mut acc, item| {
                        let _ = write!(acc, "{}/", item);
                        acc
                    })
                    .trim_end_matches('/')
                    .to_string()
            })
            .unwrap_or_else(|| "<query>".to_string());

        // Get the location of the error. We'll use just the first location for this.
        let loc = self
            .locations
            .as_ref()
            .and_then(|locations| locations.iter().next())
            .cloned()
            .unwrap_or_default();

        write!(f, "{}:{}:{}: {}", path, loc.line, loc.column, self.message)
    }
}

/// The generic shape taken by the responses of GraphQL APIs.
///
/// This will generally be used with the `ResponseData` struct from a derived module.
///
/// [Spec](https://github.com/facebook/graphql/blob/master/spec/Section%207%20--%20Response.md)
///
/// ```
/// # use serde_json::json;
/// # use serde::Deserialize;
/// # use graphql_client::GraphQLQuery;
/// # use std::error::Error;
/// #
/// # #[derive(Debug, Deserialize, PartialEq)]
/// # struct User {
/// #     id: i32,
/// # }
/// #
/// # #[derive(Debug, Deserialize, PartialEq)]
/// # struct Dog {
/// #     name: String
/// # }
/// #
/// # #[derive(Debug, Deserialize, PartialEq)]
/// # struct ResponseData {
/// #     users: Vec<User>,
/// #     dogs: Vec<Dog>,
/// # }
/// #
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use graphql_client::Response;
///
/// let body: Response<ResponseData> = serde_json::from_value(json!({
///     "data": {
///         "users": [{"id": 13}],
///         "dogs": [{"name": "Strelka"}],
///     },
///     "errors": [],
/// }))?;
///
/// let expected: Response<ResponseData> = Response {
///     data: Some(ResponseData {
///         users: vec![User { id: 13 }],
///         dogs: vec![Dog { name: "Strelka".to_owned() }],
///     }),
///     errors: Some(vec![]),
///     extensions: None,
/// };
///
/// assert_eq!(body, expected);
///
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Response<Data> {
    /// The absent, partial or complete response data.
    pub data: Option<Data>,
    /// The top-level errors returned by the server.
    pub errors: Option<Vec<Error>>,
    /// Additional extensions. Their exact format is defined by the server.
    /// See [GraphQL Response Specification](https://github.com/graphql/graphql-spec/blob/main/spec/Section%207%20--%20Response.md#response-format)
    pub extensions: Option<HashMap<String, serde_json::Value>>,
}

/// Hidden module for types used by the codegen crate.
#[doc(hidden)]
pub mod _private {
    pub use ::serde;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn graphql_error_works_with_just_message() {
        let err = json!({
            "message": "I accidentally your whole query"
        });

        let deserialized_error: Error = serde_json::from_value(err).unwrap();

        assert_eq!(
            deserialized_error,
            Error {
                message: "I accidentally your whole query".to_string(),
                locations: None,
                path: None,
                extensions: None,
            }
        )
    }

    #[test]
    fn full_graphql_error_deserialization() {
        let err = json!({
            "message": "I accidentally your whole query",
            "locations": [{ "line": 3, "column": 13}, {"line": 56, "column": 1}],
            "path": ["home", "alone", 3, "rating"]
        });

        let deserialized_error: Error = serde_json::from_value(err).unwrap();

        assert_eq!(
            deserialized_error,
            Error {
                message: "I accidentally your whole query".to_string(),
                locations: Some(vec![
                    Location {
                        line: 3,
                        column: 13,
                    },
                    Location {
                        line: 56,
                        column: 1,
                    },
                ]),
                path: Some(vec![
                    PathFragment::Key("home".to_owned()),
                    PathFragment::Key("alone".to_owned()),
                    PathFragment::Index(3),
                    PathFragment::Key("rating".to_owned()),
                ]),
                extensions: None,
            }
        )
    }

    #[test]
    fn full_graphql_error_with_extensions_deserialization() {
        let err = json!({
            "message": "I accidentally your whole query",
            "locations": [{ "line": 3, "column": 13}, {"line": 56, "column": 1}],
            "path": ["home", "alone", 3, "rating"],
            "extensions": {
                "code": "CAN_NOT_FETCH_BY_ID",
                "timestamp": "Fri Feb 9 14:33:09 UTC 2018"
            }
        });

        let deserialized_error: Error = serde_json::from_value(err).unwrap();

        let mut expected_extensions = HashMap::new();
        expected_extensions.insert("code".to_owned(), json!("CAN_NOT_FETCH_BY_ID"));
        expected_extensions.insert("timestamp".to_owned(), json!("Fri Feb 9 14:33:09 UTC 2018"));
        let expected_extensions = Some(expected_extensions);

        assert_eq!(
            deserialized_error,
            Error {
                message: "I accidentally your whole query".to_string(),
                locations: Some(vec![
                    Location {
                        line: 3,
                        column: 13,
                    },
                    Location {
                        line: 56,
                        column: 1,
                    },
                ]),
                path: Some(vec![
                    PathFragment::Key("home".to_owned()),
                    PathFragment::Key("alone".to_owned()),
                    PathFragment::Index(3),
                    PathFragment::Key("rating".to_owned()),
                ]),
                extensions: expected_extensions,
            }
        )
    }

    #[test]
    fn persisted_query_body_serialization() {
        #[derive(serde::Serialize)]
        struct TestVariables {
            id: String,
        }

        let body = PersistedQueryBody::new(
            TestVariables {
                id: "123".to_string(),
            },
            "MyQuery",
            "abc123def456789",
        );

        let serialized = serde_json::to_value(&body).unwrap();

        assert_eq!(
            serialized,
            json!({
                "operationName": "MyQuery",
                "variables": {"id": "123"},
                "extensions": {
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "abc123def456789"
                    }
                }
            })
        );
    }

    #[test]
    fn persisted_query_body_has_no_query_field() {
        #[derive(serde::Serialize)]
        struct EmptyVariables {}

        let body = PersistedQueryBody::new(EmptyVariables {}, "MyQuery", "abc123");

        let serialized = serde_json::to_string(&body).unwrap();
        assert!(
            !serialized.contains("\"query\""),
            "Persisted query body should not contain a 'query' field"
        );
    }
}
