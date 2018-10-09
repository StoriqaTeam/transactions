use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Debug, Display};

use diesel::sql_types::VarChar;
use serde::{Serialize, Serializer};

use validator::{Validate, ValidationError, ValidationErrors};

#[derive(Deserialize, FromSqlRow, AsExpression, Clone, Default, PartialEq, Eq, Hash)]
#[sql_type = "VarChar"]
pub struct AuthenticationToken(String);
derive_newtype_sql!(authentication_token, VarChar, AuthenticationToken, AuthenticationToken);
mask_logs!(AuthenticationToken);

const MASK: &str = "********";

impl Serialize for AuthenticationToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(MASK)
    }
}

impl Validate for AuthenticationToken {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let token_len = self.0.len();
        let mut errors = ValidationErrors::new();
        if token_len < 8 || token_len > 30 {
            let error = ValidationError {
                code: Cow::from("len"),
                message: Some(Cow::from("Authentication Token should be between 8 and 30 symbols")),
                params: HashMap::new(),
            };
            errors.add("token", error);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl AuthenticationToken {
    pub fn new(token: String) -> Self {
        AuthenticationToken(token)
    }

    pub fn raw(&self) -> &str {
        &self.0
    }
}
