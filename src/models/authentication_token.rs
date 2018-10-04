use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Debug, Display};
use std::io::Write;

use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::VarChar;
use serde::{Serialize, Serializer};

use validator::{Validate, ValidationErrors};

#[derive(Deserialize, FromSqlRow, AsExpression, Clone)]
#[sql_type = "VarChar"]
pub struct AuthenticationToken(String);

impl FromSql<VarChar, Pg> for AuthenticationToken {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        FromSql::<VarChar, Pg>::from_sql(data).map(AuthenticationToken)
    }
}

impl ToSql<VarChar, Pg> for AuthenticationToken {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        ToSql::<VarChar, Pg>::to_sql(&self.0, out)
    }
}

const MASK: &str = "********";

impl Debug for AuthenticationToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(MASK)
    }
}

impl Display for AuthenticationToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(MASK)
    }
}

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
        if token_len < 1 || token_len > 40 {
            let error = validator::ValidationError {
                code: Cow::from("len"),
                message: Some(Cow::from("Password should be between 8 and 30 symbols")),
                params: HashMap::new(),
            };
            errors.add("password", error);
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
}
