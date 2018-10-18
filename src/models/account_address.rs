use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Display};

use diesel::sql_types::VarChar;
use uuid::Uuid;
use validator::{Validate, ValidationError, ValidationErrors};

use models::*;

#[derive(Deserialize, FromSqlRow, AsExpression, Clone, PartialEq, Eq, Hash, Serialize, Debug)]
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

impl Default for AccountAddress {
    fn default() -> Self {
        AccountAddress(Uuid::new_v4().to_string())
    }
}

impl Display for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

#[derive(Debug, Validate, Clone, Serialize)]
pub struct CreateAccountAddress {
    pub id: Uuid,
    pub currency: Currency,
}

impl Default for CreateAccountAddress {
    fn default() -> Self {
        Self {
            currency: Currency::Eth,
            id: Uuid::new_v4(),
        }
    }
}

impl From<CreateAccount> for CreateAccountAddress {
    fn from(acc: CreateAccount) -> Self {
        Self {
            id: Uuid::new_v4(),
            currency: acc.currency,
        }
    }
}
