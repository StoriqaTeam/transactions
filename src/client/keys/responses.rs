use models::*;

#[derive(Debug, Deserialize, Clone)]
pub struct CreateAccountAddressResponse {
    pub account_address: AccountAddress,
}
