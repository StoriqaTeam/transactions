use std::borrow::Cow;
use std::collections::HashMap;

use diesel::sql_types::VarChar;
use validator::{Validate, ValidationError, ValidationErrors};

use models::*;

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

#[derive(Debug, Validate, Clone, Serialize)]
pub struct CreateAccountAddress {
    pub id: AccountId,
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: String,
    pub user_id: UserId,
    pub currency: Currency,
}

impl Default for CreateAccountAddress {
    fn default() -> Self {
        Self {
            id: AccountId::default(),
            name: String::default(),
            user_id: UserId::default(),
            currency: Currency::Eth,
        }
    }
}

impl From<(CreateAccountAddress, AccountAddress)> for NewAccount {
    fn from(req: (CreateAccountAddress, AccountAddress)) -> Self {
        Self {
            id: req.0.id,
            name: req.0.name,
            currency: req.0.currency,
            user_id: req.0.user_id,
            balance: Amount::default(),
            account_address: req.1,
        }
    }
}
