use std::time::SystemTime;

use models::*;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsersResponse {
    pub id: UserId,
    pub name: String,
    pub authentication_token: AuthenticationToken,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<User> for UsersResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            authentication_token: user.authentication_token,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountsResponse {
    pub id: AccountId,
    pub user_id: UserId,
    pub balance: Amount,
    pub currency: Currency,
    pub account_address: AccountAddress,
    pub name: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<Account> for AccountsResponse {
    fn from(account: Account) -> Self {
        Self {
            id: account.id,
            user_id: account.user_id,
            balance: account.balance,
            currency: account.currency,
            account_address: account.account_address,
            name: account.name,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    pub balance: Amount,
    pub currency: Currency,
}

impl From<Balance> for BalanceResponse {
    fn from(balance: Balance) -> Self {
        Self {
            balance: balance.balance,
            currency: balance.currency,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BalancesResponse {
    #[serde(flatten)]
    pub data: Vec<BalanceResponse>,
}

impl From<Vec<Balance>> for BalancesResponse {
    fn from(balances: Vec<Balance>) -> Self {
        Self {
            data: balances.into_iter().map(From::from).collect(),
        }
    }
}
