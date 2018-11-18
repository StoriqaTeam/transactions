## Local testing with Docker Compose

- Build transactions binary: `cargo build`
- Fire up Docker: `docker-compose up`

## Accounts and transactions

Each account has one of two `kinds` - either `Dr` (debit) or `Cr` (credit). When a user creates an account - two accounts of types `Dr` and `Cr` are created for the same wallet. `Cr` account tracks user's balance, `Dr` account tracks out payment system balance on the blockchain.

Each transaction has a form:

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 1  | ffxa          | ofmd          | 5     | done    | 0xcdef76796      |
| 2  | ghxx          | cbff          | 10    | done    |                  |
|    |               |               |       |         |                  |

### Transaction example cases

1) User creates an ETH account and makes a deposit.

When a user creates an account, blockchain address is generated `0x26df8a` and two accounts are created.

__Accounts__

| id | kind | currency | blockchain_address | name             |
|----|------|----------|--------------------|------------------|
| 35 | dr   | eth      | 0x26df8a           |                  |
| 36 | cr   | eth      | 0x26df8a           | My ether account |
|    |      |          |                    |                  |


When a 9 `wei` deposit from blockchain arrives, the following transaction is created

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 1  | 35            | 36            | 9     | done    | 0xcdef76796      |
|    |               |               |       |         |                  |




&nbsp;

2) User makes a 3 `wei` withdrawal

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 2  | 36            | 35            | 3     | done    | 0xc58dae555      |
|    |               |               |       |         |                  |

&nbsp;

3) User makes a 5 `wei` transfer from own account to other user's account with id `49`

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 3  | 36            | 49            | 5     | done    |                  |
|    |               |               |       |         |                  |

### Blockchain status changing

# Ethereum
< $20 / $200 - 0 conf
< $50 / $200 - 1 conf
< $200 / $200 - 2 conf
< $500 / $200 - 3 conf
< $1000 / $200 - 4 conf
< $2000 / $200 - 5 conf
< $3000 / $200 - 6 conf
< $5000 / $200 - 8 conf
> $5000 / $200 - 12 conf

# Bitcoin

< $100 / $6400 - 0 conf
< $500 / $6400 - 1 conf
< $1000 / $6400 - 2 conf
> $1000 / $6400 - 3 conf

### Operations example cases

1) User's account balance with id `36`

Balance

`=`

sum of transactions values with cr_account_id == `36`

`-`

sum of transactions values with dr_account_id == `36`

&nbsp;

2) Withdrawal. Once a user picks up an amount for withdrawal you pick any `dr` account that has this amount (or several accounts that is summed to total withdrawal value) and make a blockchain transaction + `pending` transaction in our system. Once blockchain transaction is confirmed, you change the status of our transaction to `done`.

E.g. we have accounts

__Accounts__

| id | kind | currency | blockchain_address | name                  |
|----|------|----------|--------------------|-----------------------|
| 67 | dr   | eth      | 0x87ff89           | Random acc with money |
| 36 | cr   | eth      | 0x26df8a           | My ether account      |
|    |      |          |                    |                       |

And a user wants to withdraw 5 `wei` from account `36`, and we know that both of these accounts have enough balance. Then we create a transaction in blockchain (meaning now we have a blockchain tx_id, e.g. `0x4f78cf324`) and send it to blockchain. At the same time we create a transaction in out system:

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 55 | 36            | 67            | 5     | pending | 0x4f78cf324      |
|    |               |               |       |         |                  |

Once our system picks this tx_id from blockchain as confirmed, we switch this transaction to `done`.

3) Withdrawal fees - in reality blockchain transaction will also have fees. That means that we also need to impose fees on transactions. Let's say we're using previous example and our fee is 2 `wei` and blockchain fee is 1 `wei` (that will be known at the time of blockchain confirmation). Let's say we the following accounts (incl. special `fees` account)

__Accounts__

| id | kind | currency | blockchain_address | name                  |
|----|------|----------|--------------------|-----------------------|
| 1  | cr   | eth      | 0xfad678           | System fees acc       |
| 67 | dr   | eth      | 0x87ff89           | Random acc with money |
| 36 | cr   | eth      | 0x26df8a           | My ether account      |
|    |      |          |                    |                       |
|    |      |          |                    |                       |

Note that `fees` acc generally don't need a blockchain address, but if we screwed up somehow and earned less fees that wasted on withdrawals then we'll need a way to top up this account.

Then on step 1 we have

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 55 | 36            | 67            | 5     | pending | 0x4f78cf324      |
| 56 | 36            | 1             | 2     | pending | 0x4f78cf324      |
|    |               |               |       |         |                  |

Once the tx `0x4f78cf324` arrives we learn that actual fee is 1 `wei`, we update the transactions table in the following way.

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 55 | 36            | 67            | 5     | done    | 0x4f78cf324      |
| 56 | 36            | 1             | 2     | done    | 0x4f78cf324      |
| 57 | 1             | 67            | 1     | done    | 0x4f78cf324      |
|    |               |               |       |         |                  |


TODO: We need to address here the case of having transaction from 2 and more wallets, since it has bigger blockchain costs.

#### Transfers with different currencies

Each transfer to account with different currency is decomposed into
  1. Conversion between own accounts
  2. Transfer with the same currency

So the transfer basically boils down to converting between own accounts. Let's consider a case with `BTC` and `ETH`. We need to have 4 special accounts in out system:

__Accounts__

| id | kind | currency | blockchain_address | name                      |
|----|------|----------|--------------------|---------------------------|
| 2  | cr   | eth      | 0xfad678           | System eth liquidity acc  |
| 3  | dr   | eth      | 0xfad678           | System eth liquidity acc  |
| 4  | cr   | btc      | 5jdfjgljdlkjg      | System btc liquidity acc  |
| 5  | dr   | btc      | 5jdfjgljdlkjg      | System btc liquidity acc  |
|    |      |          |                    |                           |


Assuming current exchange rate is 5 ETH/BTC, we need have some balances, e.g. 10 btc and 50 eth. Same amounts must be stored on our exchange. Let's consider a user with two accounts

__Accounts__

| id | kind | currency | blockchain_address | name                      |
|----|------|----------|--------------------|---------------------------|
| 88 | cr   | eth      | 0xfad678           | My eth account            |
| 90 | cr   | btc      | 5jvuisuslfu        | My btc account            |

Once a user tries to make an exchange, a fixed exchange rate is given to him with expiration time, e.g. 4 ETH / BTC expiring in 5 minutes. Then if a user is making a transfer before expiration of 4 ETH into 1 BTC, we have the following trancations:

__Transactions__

| id | dr_account_id | cr_account_id | value | status  | blockchain_tx_id |
|----|---------------|---------------|-------|---------|------------------|
| 90 | 88            | 2             | 4     | done    |                  |
| 91 | 4             | 90            | 1     | done    |                  |

Since we 'lost' 1 BTC from our systems accounts, we need to send immediately a call to exchange client to change ETH to 1 BTC (hopefully at better exchange rate), so that the totals of our systems accounts and exchange accounts are always growing.

At the time when our systems account balance falls below some predefined threshold we make a deposit to it from our exchange.

#### Deferred transactions

Deferred transacations execute when certain conditions are met. Currently two types of conditions are available:
 1. Time condition - transaction is executed when certain time elapses
 2. Balance condition - transaction is executed when certain account balance reaches some thresholds

__Use cases__

1. If you want to create invoice - a special system account is created with a deferred transactions - once required total amount arrived, transfer it to a specific set of accounts in parts. Additional time condition to expire after certain date is imposed.

2. When you want to hold amount of money for some time, you create a deferred by time condition transaction

TODO: What happens when you transfer more money than required to invoice account
TODO: Cancelling deferred transactions

#### IMPORTANT: Invariants that must hold at all times

These invariants must always hold. They must be constantly monitored and if some of them are violated, it's most likely WE'RE HACKED, so we need instant alert, or maybe automatic suspension of all operations.

1. Each `Dr` account balance must exactly equal to our balance on this blockchain wallet.

2. Each `Dr` and `Cr` account balance must be greater than zero. If that's not true - we're hacked. __VERY IMPORTANT: This invariant must uphold using database constraints on each new transaction__.

3. The sum of total dr accounts (our blockchain assets) must equal to the sum of total cr accounts (our liabilities to users). This follows by design from `1` and `2`

4. Each balance decrease in blockchain must have a corresponding pending transaction in our system.

5. We need to make sure that we have enough balance of `fees` account, and be alerted if it's falling below some level. This means we're loosing money on fees.


### Fees management

Currently eth_fees_account is responsible for managing stq transactions.
You need to make couple of things:
1) Prefill this account for fees in ether (required for approval of stq accounts)
2) Write the address of this acccount in keystore config (stq_controller_address = "..."), so that stq withdrawals are made on its behalf
