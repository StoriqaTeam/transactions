use std::str::FromStr;

use models::*;

#[derive(Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ReceiptType {
    Account,
    Address,
}

#[derive(Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Receipt(String);

impl Receipt {
    pub fn to_account_id(&self) -> Result<AccountId, ()> {
        AccountId::from_str(&self.0).map_err(|_| ())
    }
    pub fn to_account_address(&self) -> AccountAddress {
        AccountAddress::new(self.0.clone())
    }
}

pub enum CrReceiptType {
    Account(Account),
    Address(AccountAddress),
}
