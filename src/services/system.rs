use std::sync::Arc;

use super::error::*;
use config::Config;
use models::*;
use prelude::*;
use repos::AccountsRepo;

pub trait SystemService: Send + Sync + 'static {
    fn get_system_transfer_account(&self, currency: Currency) -> Result<Account, Error>;
    fn get_system_liquidity_account(&self, currency: Currency) -> Result<Account, Error>;
    fn get_system_fees_account(&self, currency: Currency) -> Result<Account, Error>;
    fn get_system_fees_account_dr(&self, currency: Currency) -> Result<Account, Error>;
}

#[derive(Clone)]
pub struct SystemServiceImpl {
    accounts_repo: Arc<AccountsRepo>,
    config: Arc<Config>,
}

impl SystemServiceImpl {
    pub fn new(accounts_repo: Arc<AccountsRepo>, config: Arc<Config>) -> Self {
        Self { accounts_repo, config }
    }
}

impl SystemService for SystemServiceImpl {
    fn get_system_transfer_account(&self, currency: Currency) -> Result<Account, Error> {
        let acc_id = match currency {
            Currency::Btc => self.config.system.btc_transfer_account_id,
            Currency::Eth => self.config.system.eth_transfer_account_id,
            Currency::Stq => self.config.system.stq_transfer_account_id,
        };
        let acc = self
            .accounts_repo
            .get(acc_id)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound))?;
        Ok(acc)
    }

    fn get_system_liquidity_account(&self, currency: Currency) -> Result<Account, Error> {
        let acc_id = match currency {
            Currency::Btc => self.config.system.btc_liquidity_account_id,
            Currency::Eth => self.config.system.eth_liquidity_account_id,
            Currency::Stq => self.config.system.stq_liquidity_account_id,
        };
        let acc = self
            .accounts_repo
            .get(acc_id)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound))?;
        Ok(acc)
    }

    fn get_system_fees_account(&self, currency: Currency) -> Result<Account, Error> {
        let acc_id = match currency {
            Currency::Btc => self.config.system.btc_fees_account_id,
            Currency::Eth => self.config.system.eth_fees_account_id,
            Currency::Stq => self.config.system.stq_fees_account_id,
        };
        let acc = self
            .accounts_repo
            .get(acc_id)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound))?;
        Ok(acc)
    }

    fn get_system_fees_account_dr(&self, currency: Currency) -> Result<Account, Error> {
        let acc_id = match currency {
            Currency::Btc => self.config.system.btc_fees_account_id,
            Currency::Eth => self.config.system.eth_fees_account_id,
            Currency::Stq => self.config.system.stq_fees_account_id,
        };
        let dr_acc_id = acc_id.derive_system_dr_id();
        let acc = self
            .accounts_repo
            .get(dr_acc_id)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound))?;
        Ok(acc)
    }
}
