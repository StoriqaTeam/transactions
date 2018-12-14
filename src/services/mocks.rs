use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::auth::AuthService;
use super::error::*;
use super::system::SystemService;
use super::ServiceFuture;
use models::*;
use prelude::*;

pub struct AuthServiceMock {
    users: HashMap<AuthenticationToken, User>,
}

impl AuthServiceMock {
    pub fn new(allowed_tokens: Vec<(AuthenticationToken, UserId)>) -> Self {
        let mut users = HashMap::new();
        for (token, id) in allowed_tokens {
            let mut user = User::default();
            user.authentication_token = token.clone();
            user.id = id;
            users.insert(token, user);
        }
        AuthServiceMock { users }
    }
}

impl AuthService for AuthServiceMock {
    fn authenticate(&self, token: AuthenticationToken) -> ServiceFuture<User> {
        Box::new(
            self.users
                .get(&token)
                .map(|x| x.clone())
                .ok_or(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized))
                .into_future(),
        )
    }
}

#[derive(Clone)]
pub struct SystemServiceMock {
    data: Arc<Mutex<HashMap<String, Account>>>,
}

impl SystemServiceMock {
    pub fn new(
        transfer_accounts: [Account; 3],
        liquidity_accounts: [Account; 3],
        fees_accounts: [Account; 3],
        fees_accounts_dr: [Account; 3],
    ) -> Self {
        let mut accounts = HashMap::new();
        accounts.insert("btc_transfer_account_id".to_string(), transfer_accounts[0].clone());
        accounts.insert("eth_transfer_account_id".to_string(), transfer_accounts[1].clone());
        accounts.insert("stq_transfer_account_id".to_string(), transfer_accounts[2].clone());

        accounts.insert("btc_liquidity_account_id".to_string(), liquidity_accounts[0].clone());
        accounts.insert("eth_liquidity_account_id".to_string(), liquidity_accounts[1].clone());
        accounts.insert("stq_liquidity_account_id".to_string(), liquidity_accounts[2].clone());

        accounts.insert("btc_fees_account_id".to_string(), fees_accounts[0].clone());
        accounts.insert("eth_fees_account_id".to_string(), fees_accounts[1].clone());
        accounts.insert("stq_fees_account_id".to_string(), fees_accounts[2].clone());

        accounts.insert("btc_fees_account_id_dr".to_string(), fees_accounts_dr[0].clone());
        accounts.insert("eth_fees_account_id_dr".to_string(), fees_accounts_dr[1].clone());
        accounts.insert("stq_fees_account_id_dr".to_string(), fees_accounts_dr[2].clone());

        Self {
            data: Arc::new(Mutex::new(accounts)),
        }
    }
}

impl SystemService for SystemServiceMock {
    fn get_system_transfer_account(&self, currency: Currency) -> Result<Account, Error> {
        let data = self.data.lock().unwrap();
        let acc_id = match currency {
            Currency::Btc => "btc_transfer_account_id",
            Currency::Eth => "eth_transfer_account_id",
            Currency::Stq => "stq_transfer_account_id",
        };
        let acc = data.get(acc_id).unwrap();
        Ok(acc.clone())
    }

    fn get_system_liquidity_account(&self, currency: Currency) -> Result<Account, Error> {
        let data = self.data.lock().unwrap();
        let acc_id = match currency {
            Currency::Btc => "btc_liquidity_account_id",
            Currency::Eth => "eth_liquidity_account_id",
            Currency::Stq => "stq_liquidity_account_id",
        };
        let acc = data.get(acc_id).unwrap();
        Ok(acc.clone())
    }

    fn get_system_fees_account(&self, currency: Currency) -> Result<Account, Error> {
        let data = self.data.lock().unwrap();
        let acc_id = match currency {
            Currency::Btc => "btc_fees_account_id",
            Currency::Eth => "eth_fees_account_id",
            Currency::Stq => "stq_fees_account_id",
        };
        let acc = data.get(acc_id).unwrap();
        Ok(acc.clone())
    }

    fn get_system_fees_account_dr(&self, currency: Currency) -> Result<Account, Error> {
        let data = self.data.lock().unwrap();
        let acc_id = match currency {
            Currency::Btc => "btc_fees_account_id_dr",
            Currency::Eth => "eth_fees_account_id_dr",
            Currency::Stq => "stq_fees_account_id_dr",
        };
        let acc = data.get(acc_id).unwrap();
        Ok(acc.clone())
    }
}
