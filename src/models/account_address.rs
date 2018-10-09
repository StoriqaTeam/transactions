use std::borrow::Cow;
use std::collections::HashMap;

use diesel::sql_types::VarChar;

use validator::{Validate, ValidationError, ValidationErrors};

#[derive(Deserialize, FromSqlRow, AsExpression, Clone, Default, PartialEq, Eq, Hash, Serialize, Debug)]
#[sql_type = "VarChar"]
pub struct AccountAddress(String);
derive_newtype_sql!(account_address, VarChar, AccountAddress, AccountAddress);

impl Validate for AccountAddress {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let token_len = self.0.len();
        let mut errors = ValidationErrors::new();
        if token_len < 1 || token_len > 40 {
            let error = ValidationError {
                code: Cow::from("len"),
                message: Some(Cow::from("Account Address should be between 1 and 40 symbols")),
                params: HashMap::new(),
            };
            errors.add("account_address", error);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl AccountAddress {
    pub fn new(token: String) -> Self {
        AccountAddress(token)
    }

    pub fn raw(&self) -> &str {
        &self.0
    }
}
