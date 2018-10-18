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
    pub currency: Currency,
    pub address: AccountAddress,
    pub name: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<Account> for AccountsResponse {
    fn from(account: Account) -> Self {
        Self {
            id: account.id,
            user_id: account.user_id,
            currency: account.currency,
            address: account.address,
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionsResponse {
    pub id: TransactionId,
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub hold_until: Option<SystemTime>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<Transaction> for TransactionsResponse {
    fn from(transaction: Transaction) -> Self {
        Self {
            id: transaction.id,
            user_id: transaction.user_id,
            dr_account_id: transaction.dr_account_id,
            cr_account_id: transaction.cr_account_id,
            currency: transaction.currency,
            value: transaction.value,
            status: transaction.status,
            blockchain_tx_id: transaction.blockchain_tx_id,
            hold_until: transaction.hold_until,
            created_at: transaction.created_at,
            updated_at: transaction.updated_at,
        }
    }
}
