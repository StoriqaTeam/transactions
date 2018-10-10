use std::time::SystemTime;

use models::*;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostUsersRequest {
    pub id: UserId,
    pub name: String,
    pub authentication_token: AuthenticationToken,
}

impl From<PostUsersRequest> for NewUser {
    fn from(req: PostUsersRequest) -> Self {
        Self {
            id: req.id,
            name: req.name,
            authentication_token: req.authentication_token,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PutUsersRequest {
    pub name: Option<String>,
    pub authentication_token: Option<AuthenticationToken>,
}

impl From<PutUsersRequest> for UpdateUser {
    fn from(req: PutUsersRequest) -> Self {
        Self {
            name: req.name,
            authentication_token: req.authentication_token,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostAccountsRequest {
    pub id: AccountId,
    pub user_id: UserId,
    pub currency: Currency,
    pub name: String,
}

impl From<PostAccountsRequest> for CreateAccount {
    fn from(req: PostAccountsRequest) -> Self {
        Self {
            id: req.id,
            name: req.name,
            currency: req.currency,
            user_id: req.user_id,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PutAccountsRequest {
    pub name: Option<String>,
}

impl From<PutAccountsRequest> for UpdateAccount {
    fn from(req: PutAccountsRequest) -> Self {
        Self { name: req.name }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUsersAccountsParams {
    pub limit: i64,
    pub offset: AccountId,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostTransactionsRequest {
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub hold_until: Option<SystemTime>,
}

impl From<PostTransactionsRequest> for CreateTransactionLocal {
    fn from(req: PostTransactionsRequest) -> Self {
        Self {
            user_id: req.user_id,
            dr_account_id: req.dr_account_id,
            cr_account_id: req.cr_account_id,
            currency: req.currency,
            value: req.value,
            hold_until: req.hold_until,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PutTransactionsRequest {
    pub status: TransactionStatus,
}

impl From<PutTransactionsRequest> for TransactionStatus {
    fn from(req: PutTransactionsRequest) -> Self {
        req.status
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUsersTransactionsParams {
    pub limit: i64,
    pub offset: TransactionId,
}
