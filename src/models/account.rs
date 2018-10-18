use std::time::SystemTime;

use validator::Validate;

use models::*;
use schema::accounts;

#[derive(Debug, Queryable, Clone)]
pub struct Account {
    pub id: AccountId,
    pub user_id: UserId,
    pub currency: Currency,
    pub address: AccountAddress,
    pub name: Option<String>,
    pub kind: AccountKind,
    pub balance: Amount,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: AccountId::generate(),
            user_id: UserId::generate(),
            currency: Currency::Eth,
            address: AccountAddress::default(),
            name: None,
            kind: AccountKind::Cr,
            balance: Amount::default(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }
}

impl From<NewAccount> for Account {
    fn from(new_account: NewAccount) -> Self {
        Self {
            id: new_account.id,
            name: new_account.name,
            user_id: new_account.user_id,
            currency: new_account.currency,
            address: new_account.address,
            kind: new_account.kind,
            ..Default::default()
        }
    }
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "accounts"]
pub struct NewAccount {
    pub id: AccountId,
    pub user_id: UserId,
    pub currency: Currency,
    #[validate]
    pub address: AccountAddress,
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: Option<String>,
    pub kind: AccountKind,
}

impl Default for NewAccount {
    fn default() -> Self {
        Self {
            id: AccountId::generate(),
            name: None,
            user_id: UserId::generate(),
            currency: Currency::Eth,
            address: AccountAddress::default(),
            kind: AccountKind::Cr,
        }
    }
}

impl NewAccount {
    pub fn create_debit(&self) -> Self {
        Self {
            id: AccountId::generate(),
            name: None,
            user_id: self.user_id,
            currency: self.currency,
            address: self.address.clone(),
            kind: AccountKind::Dr,
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

#[derive(Debug, Clone)]
pub struct Balance {
    pub currency: Currency,
    pub balance: Amount,
}

impl Balance {
    pub fn new(currency: Currency, balance: Amount) -> Self {
        Self { currency, balance }
    }
}

impl From<Account> for Balance {
    fn from(acc: Account) -> Self {
        Self {
            currency: acc.currency,
            balance: acc.balance,
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct CreateAccount {
    pub id: AccountId,
    pub user_id: UserId,
    pub currency: Currency,
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: String,
}

impl Default for CreateAccount {
    fn default() -> Self {
        Self {
            id: AccountId::generate(),
            user_id: UserId::generate(),
            currency: Currency::Eth,
            name: String::default(),
        }
    }
}

impl From<(CreateAccount, AccountAddress)> for NewAccount {
    fn from(create: (CreateAccount, AccountAddress)) -> Self {
        Self {
            id: create.0.id,
            name: Some(create.0.name),
            user_id: create.0.user_id,
            currency: create.0.currency,
            kind: AccountKind::Cr,
            address: create.1,
        }
    }
}
