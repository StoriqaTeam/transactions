use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::FromIterator;

use failure::Fail;
use futures::prelude::*;
use hyper::Response;
use serde::Deserialize;
use serde::Serialize;
use serde_json;

use super::error::*;
use super::ControllerFuture;

pub fn parse_body<T>(body: Vec<u8>) -> impl Future<Item = T, Error = Error> + Send
where
    T: for<'de> Deserialize<'de> + Send,
{
    String::from_utf8(body.clone())
        .map_err(ectx!(ErrorContext::RequestUTF8, ErrorKind::BadRequest => body))
        .into_future()
        .and_then(|string| serde_json::from_str::<T>(&string).map_err(ectx!(ErrorContext::RequestJson, ErrorKind::BadRequest => string)))
}

pub fn response_with_model<M>(model: &M) -> ControllerFuture
where
    M: Debug + Serialize,
{
    Box::new(
        serde_json::to_string(&model)
            .map_err(ectx!(ErrorContext::ResponseJson, ErrorKind::Internal => model))
            .into_future()
            .map(|text| {
                Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(text.into())
                    .unwrap()
            }),
    )
}

/// Splits query string to key-value pairs. See `macros::parse_query` for more sophisticated parsing.
// TODO: Cover more complex cases, e.g. `from=count=10`
pub fn query_params(query: &str) -> HashMap<&str, &str> {
    HashMap::from_iter(query.split('&').map(|pair| {
        let mut params = pair.split('=');
        (params.next().unwrap(), params.next().unwrap_or(""))
    }))
}
