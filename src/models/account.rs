use std::time::SystemTime;

use validator::Validate;

use models::*;
use schema::accounts;

#[derive(Debug, Queryable, Clone)]
pub struct Account {
    pub id: AccountId,
    pub user_id: UserId,
    pub balance: Amount,
    pub currency: Currency,
    pub account_address: AccountAddress,
    pub name: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: AccountId::default(),
            name: String::default(),
            user_id: UserId::default(),
            balance: Amount::default(),
            currency: Currency::Eth,
            account_address: AccountAddress::default(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "accounts"]
pub struct NewAccount {
    pub id: AccountId,
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: String,
    pub user_id: UserId,
    pub balance: Amount,
    pub currency: Currency,
    #[validate]
    pub account_address: AccountAddress,
}

impl Default for NewAccount {
    fn default() -> Self {
        Self {
            id: AccountId::default(),
            name: String::default(),
            user_id: UserId::default(),
            balance: Amount::default(),
            currency: Currency::Eth,
            account_address: AccountAddress::default(),
        }
    }
}

/// Payload for updating accounts
#[derive(Debug, Insertable, Validate, AsChangeset, Clone, Default)]
#[table_name = "accounts"]
pub struct UpdateAccount {
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: Option<String>,
}
