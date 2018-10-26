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
    pub balance: Amount,
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
            balance: account.balance,
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
    pub from: Vec<TransactionAddressInfo>,
    pub to: TransactionAddressInfo,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<TransactionOut> for TransactionsResponse {
    fn from(transaction: TransactionOut) -> Self {
        Self {
            id: transaction.id,
            from: transaction.from,
            to: transaction.to,
            currency: transaction.currency,
            value: transaction.value,
            fee: transaction.fee,
            status: transaction.status,
            blockchain_tx_id: transaction.blockchain_tx_id,
            created_at: transaction.created_at,
            updated_at: transaction.updated_at,
        }
    }
}
