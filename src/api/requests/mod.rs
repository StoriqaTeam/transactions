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
        Self {
            name: req.name,
            erc20_approved: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUsersAccountsParams {
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostTransactionsRequest {
    pub id: TransactionId,
    pub user_id: UserId,
    pub from: AccountId,
    pub to: Recepient,
    pub to_type: RecepientType,
    pub to_currency: Currency,
    pub value: Amount,
    pub value_currency: Currency,
    pub fee: Amount,
    pub exchange_id: Option<ExchangeId>,
    pub exchange_rate: Option<f64>,
}

impl From<PostTransactionsRequest> for CreateTransactionInput {
    fn from(req: PostTransactionsRequest) -> Self {
        let PostTransactionsRequest {
            id,
            user_id,
            from,
            to,
            to_type,
            to_currency,
            value,
            value_currency,
            fee,
            exchange_id,
            exchange_rate,
        } = req;

        Self {
            id,
            user_id,
            from,
            to,
            to_type,
            to_currency,
            value,
            value_currency,
            fee,
            exchange_id,
            exchange_rate,
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
    pub offset: i64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostFeesRequest {
    pub from_currency: Currency,
    pub to_currency: Currency,
}

impl From<PostFeesRequest> for GetFees {
    fn from(req: PostFeesRequest) -> Self {
        Self {
            from_currency: req.from_currency,
            to_currency: req.to_currency,
        }
    }
}
