use chrono::NaiveDateTime;

use models::*;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsersResponse {
    pub id: UserId,
    pub name: String,
    pub authentication_token: AuthenticationToken,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub address: BlockchainAddress,
    pub name: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub account: Account,
}

impl From<AccountWithBalance> for BalanceResponse {
    fn from(balance: AccountWithBalance) -> Self {
        Self {
            balance: balance.balance,
            account: balance.account,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BalancesResponse {
    #[serde(flatten)]
    pub data: Vec<BalanceResponse>,
}

impl From<Vec<AccountWithBalance>> for BalancesResponse {
    fn from(balances: Vec<AccountWithBalance>) -> Self {
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
    pub from_value: Amount,
    pub from_currency: Currency,
    pub to_value: Amount,
    pub to_currency: Currency,
    pub fee: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Vec<BlockchainTransactionId>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<TransactionOut> for TransactionsResponse {
    fn from(transaction: TransactionOut) -> Self {
        Self {
            id: transaction.id,
            from: transaction.from,
            to: transaction.to,
            from_value: transaction.from_value,
            from_currency: transaction.from_currency,
            to_value: transaction.to_value,
            to_currency: transaction.to_currency,
            fee: transaction.fee,
            status: transaction.status,
            blockchain_tx_id: transaction.blockchain_tx_id,
            created_at: transaction.created_at,
            updated_at: transaction.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeesResponse {
    pub currency: Currency,
    pub fees: Vec<Fee>,
}

impl From<Fees> for FeesResponse {
    fn from(rate: Fees) -> Self {
        Self {
            currency: rate.currency,
            fees: rate.fees,
        }
    }
}
