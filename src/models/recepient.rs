use std::str::FromStr;

use models::*;

#[derive(Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "camelCase")]
pub enum RecepientType {
    Account,
    Address,
}

#[derive(Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Recepient(String);

impl Recepient {
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
