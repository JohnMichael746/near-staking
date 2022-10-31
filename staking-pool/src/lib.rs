use std::collections::HashMap;
use std::u128;

use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata,
};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, ext_contract, near_bindgen, AccountId, Balance, PromiseOrValue,
};
use near_sdk::{Gas, PanicOnDefault};

pub const ONE_HOUR: u128 = 3600_000;
pub const ONE_DAY: u128 = 86400_000;
pub const QUARTER_DAY: u64 = 86400_000 * 90;

pub const FT_TRANSFER_GAS: Gas = Gas(10_000_000_000_000);
pub const DEPOSIT_ONE_YOCTO: Balance = 1;

#[ext_contract(ext_ft)]
trait FungibleToken {
    // change methods
    fn ft_transfer(&mut self, receiver_id: String, amount: String, memo: Option<String>);
    fn ft_transfer_call(
        &mut self,
        receiver_id: String,
        amount: String,
        memo: Option<String>,
        msg: String,
    ) -> U128;

    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128;

    fn ft_mint(&mut self, receiver_id: AccountId, amount: u128);
    fn ft_burn(&mut self, account_id: AccountId, amount: u128);

    // view methods
    fn ft_total_supply(&self) -> String;
    fn ft_balance_of(&self, account_id: String) -> String;
    fn ft_metadata(&self) -> FungibleTokenMetadata;
}

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum PoolType {
    /// staking pool
    Staking,
    /// loan pool
    Loan,
}

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum TransactionType {
    /// staking transaction
    Staking,
    /// borrow transaction
    Borrow,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct UserInfo {
    transaction_type: TransactionType, // tx type
    amount: u128,       // amount of tx
    time: u64,         // start
    paid_out: u128,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenInfo {
    token: AccountId, // tx type
    collateral_token: AccountId,       // amount of tx
    decimals: u8,         // start
    name: String,
    symbol: String,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct DepositLimiters {
    duration: u64,         // reward calculation duration
    start_time: u64,       // deposit start time for staking pool
    end_time: u64,         // deposit end time for staking pool
    limit_per_user: u128,   // limit per user
    capacity: u128,         // pool capacity
    max_utilisation: u128,  // maximum utilisation of pool
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Funds {
    balance: u128,         // pool balance
    loaned_balance: u128,   // loaned amount on loan pool
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct PoolInfo {
    pool_name: String,         // pool name
    pool_type: PoolType,       // pool type
    apy: u128,         // apy of pool
    paused: bool,         // pause flag
    quarterly_payout: bool,   // if true, claim quarterly
    unique_users: u128,         // stakers and borrowers
    token_info: TokenInfo,  // token info of pool
    funds: Funds,       // balance status of pool
    deposit_limiters: DepositLimiters,       // deposit limiter of pool
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    pool_info: Vec<PoolInfo>, // pool info
    is_pool_user: HashMap<u128, HashMap<AccountId, bool>>, // check if user in pid
    is_whitelisted: HashMap<u128, HashMap<AccountId, bool>>,    // check if user in whitelist in pid
    user_info: HashMap<u128, HashMap<AccountId, Vec<UserInfo>>>,    // user's tx array in pid
    total_user_amount_staked: HashMap<u128, HashMap<AccountId, u128>>,  // user's stake amount in pid
    total_user_amount_borrowed: HashMap<u128, HashMap<AccountId, u128>>,    // user's borrowed amount in pid
}

// init
#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        Self {
            pool_info: Vec::new(),
            is_pool_user: HashMap::new(),
            is_whitelisted: HashMap::new(),
            user_info: HashMap::new(),
            total_user_amount_staked: HashMap::new(),
            total_user_amount_borrowed: HashMap::new(),
        }
    }
}

// admin
#[near_bindgen]
impl Contract {
    pub fn set_pool_paused(&mut self, pid: u128, flag: bool) {
        self.assert_caller_allowed();
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        pool.paused = flag;
    }

    pub fn whitelist(&mut self, pid: u128, user: AccountId, status: bool) {
        self.assert_caller_allowed();
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        assert!(pool.pool_type == PoolType::Loan, "no loans from here");
        let is_whitelisted = self.is_whitelisted.get_mut(&pid).unwrap().get_mut(&user).unwrap();
        *is_whitelisted = status;
    }

    pub fn create_pool(&mut self, pool_info: PoolInfo, pool_type: PoolType) {
        self.assert_caller_allowed();
        let mut t_pool_info = pool_info.clone();

        if pool_type != PoolType::Loan {
            assert!(pool_info.deposit_limiters.start_time < pool_info.deposit_limiters.end_time, "end time should be after start time");
        }

        t_pool_info.funds.balance = 0;
        t_pool_info.funds.loaned_balance = 0;
        t_pool_info.unique_users = 0;

        self.pool_info.push(t_pool_info);
    }

    pub fn edit_pool(&mut self, pid: u128, new_pool_info: PoolInfo) {
        self.assert_caller_allowed();
        let mut t_new_pool_info = new_pool_info.clone();
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();

        t_new_pool_info.funds.balance = pool.funds.balance;
        t_new_pool_info.funds.loaned_balance = pool.funds.loaned_balance;
        t_new_pool_info.unique_users = pool.unique_users;
        t_new_pool_info.token_info.token = pool.token_info.token.clone();

        *pool = t_new_pool_info;
    }

    pub fn recover_token(&mut self, token: AccountId, amount: u128) {
        self.assert_caller_allowed();
        ext_ft::ext(token)
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_transfer(
                env::current_account_id().to_string(),
                amount.to_string(),
                Some("0".to_string()),
            );
    }
}

// external
#[near_bindgen]
impl Contract {
    fn internal_deposit_and_stake(&mut self, staker: AccountId, pid: u128, token_id: AccountId, amount: u128) {
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let transaction = self.user_info.entry(pid).or_default().entry(staker.clone()).or_default();

        assert!(!pool.paused, "Pool Paused");
        assert_eq!(pool.token_info.token, token_id, "invalid token or pool id");

        if pool.pool_type == PoolType::Staking {
            assert!(env::block_timestamp_ms() >= pool.deposit_limiters.start_time && env::block_timestamp_ms() <= pool.deposit_limiters.end_time, "deposits disabled at this time");
        }
        assert!(amount <= pool.deposit_limiters.limit_per_user, "amount exceeds limit per transaction");
        assert!(pool.funds.balance + amount <= pool.deposit_limiters.capacity, "pool capacity reached");

        let user_info = UserInfo {
            transaction_type: TransactionType::Staking,
            amount,
            time: env::block_timestamp_ms(),
            paid_out: 0
        };
        transaction.push(user_info);

        ext_ft::ext(pool.token_info.collateral_token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_mint(
                staker.clone(),
                amount
            );

        let total_user_amount_staked = self.total_user_amount_staked.entry(pid).or_default().entry(staker.clone()).or_default();
        *total_user_amount_staked = *total_user_amount_staked + amount;

        pool.funds.balance += amount;
        
        let is_pool_user = self.is_pool_user.entry(pid).or_default().entry(staker.clone()).or_default();
        if *is_pool_user == false {
            pool.unique_users += 1;
        }
        *is_pool_user = true;
    }
    
    pub fn emergency_withdraw(&mut self, pid: u128, index: usize, amount: u128) {
        let account_id = env::signer_account_id();
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let transaction = self.user_info.entry(pid).or_default().entry(account_id.clone()).or_default();

        ext_ft::ext(pool.token_info.collateral_token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_burn(
                account_id.clone(),
                amount
            );

        ext_ft::ext(pool.token_info.token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_transfer(
                account_id.clone().to_string(),
                amount.to_string(),
                Some("0".to_string()),
            );

        transaction[index].amount -= amount;
        transaction[index].time = env::block_timestamp_ms();
    }

    pub fn withdraw(&mut self, pid: u128, index: usize, amount: u128) {
        let account_id = env::signer_account_id();
        
        let temp_pool = self.pool_info.get(usize::try_from(pid).unwrap()).unwrap().clone();
        let temp_transaction = self.user_info.get(&pid).unwrap().get(&account_id).unwrap().clone();
        
        if env::block_timestamp_ms() < temp_pool.deposit_limiters.end_time {
            self.emergency_withdraw(pid, index, amount);
            return ;
        }

        assert!(temp_transaction[index].transaction_type == TransactionType::Staking, "not staked");
        assert!(amount <= temp_transaction[index].amount, "amount greater than transaction");

        if temp_pool.pool_type == PoolType::Staking {
            assert!(env::block_timestamp_ms() >=  temp_pool.deposit_limiters.end_time + temp_pool.deposit_limiters.duration, "withdrawing too early");
        } else {
            assert!(temp_pool.funds.balance >= temp_pool.funds.loaned_balance + amount, "high utilisation");
            let projected_utilisation = self._calculate_percentage(
                temp_pool.funds.loaned_balance,
                temp_pool.funds.balance - amount
            );
            assert!(projected_utilisation < temp_pool.deposit_limiters.max_utilisation, "utilisation maxed out");
        }

        self.transfer_rewards(account_id.clone(), pid, index, env::block_timestamp_ms() - temp_pool.deposit_limiters.end_time, amount);
        
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let transaction = self.user_info.entry(pid).or_default().entry(account_id.clone()).or_default();

        ext_ft::ext(pool.token_info.collateral_token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_burn(
                account_id.clone(),
                amount
            );

        ext_ft::ext(pool.token_info.token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_transfer(
                account_id.clone().to_string(),
                amount.to_string(),
                Some("0".to_string()),
            );

        transaction[index].amount -= amount;
        transaction[index].time = env::block_timestamp_ms();

        let total_user_amount_staked = self.total_user_amount_staked.entry(pid).or_default().entry(account_id.clone()).or_default();
        *total_user_amount_staked = *total_user_amount_staked - amount;

        pool.funds.balance -= amount;

        self._delete_stake_if_empty(account_id, pid, index);
    }

    pub fn borrow(&mut self, pid: u128, amount: u128) {
        let account_id = env::signer_account_id();
        assert_eq!(self.is_whitelisted.get(&pid).unwrap().get(&account_id).unwrap().clone(), true, "Only whitelisted can borrow");
        
        let temp_pool = self.pool_info.get(usize::try_from(pid).unwrap()).unwrap().clone();
        let projected_utilisation = self._calculate_percentage(
            temp_pool.funds.loaned_balance + amount,
            temp_pool.funds.balance
        );

        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let loans = self.user_info.entry(pid).or_default().entry(account_id.clone()).or_default();

        assert!(pool.pool_type == PoolType::Loan, "no loans from here");
        assert!(!pool.paused, "Pool Paused");
        assert!(pool.funds.balance > 0, "Nothing deposited");
        assert!(projected_utilisation < pool.deposit_limiters.max_utilisation, "utilisation maxed out");

        ext_ft::ext(pool.token_info.token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_transfer(
                account_id.clone().to_string(),
                amount.to_string(),
                Some("0".to_string()),
            );

        let user_info = UserInfo {
            transaction_type: TransactionType::Borrow,
            amount,
            time: env::block_timestamp_ms(),
            paid_out: 0
        };
        loans.push(user_info);

        let total_user_amount_borrowed = self.total_user_amount_borrowed.entry(pid).or_default().entry(account_id.clone()).or_default();
        *total_user_amount_borrowed = * total_user_amount_borrowed + amount;

        pool.funds.loaned_balance += amount;

        let is_pool_user = self.is_pool_user.entry(pid).or_default().entry(account_id.clone()).or_default();
        if *is_pool_user == false {
            pool.unique_users += 1;
        }
        *is_pool_user = true;
    }

    fn internal_repay(&mut self, borrower: AccountId, pid: u128, index: usize, token_id: AccountId, amount: u128, repay_amount: u128) {
        let interest = self.calculate_interest(borrower.clone(), pid, index, repay_amount);
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let transaction = self.user_info.entry(pid).or_default().entry(borrower.clone()).or_default();
        
        assert_eq!(pool.token_info.token, token_id, "invalid token or pool id");
        assert!(pool.pool_type == PoolType::Loan, "nothing borrowed from here");
        assert!(transaction[index].transaction_type == TransactionType::Borrow, "not borrwed");
        assert!(repay_amount <= transaction[index].amount, "repay amount greater than borrowed");
        assert!(amount >= repay_amount + interest, "amount less than repay amount + interest");

        transaction[index].amount -= repay_amount;
        transaction[index].time = env::block_timestamp_ms();

        let total_user_amount_borrowed = self.total_user_amount_borrowed.entry(pid).or_default().entry(borrower.clone()).or_default();
        *total_user_amount_borrowed = * total_user_amount_borrowed - amount;

        pool.funds.loaned_balance -= amount;

        self._delete_stake_if_empty(borrower, pid, index);
    }

    pub fn claim_quarterly_payout(&mut self, pid: u128, index: usize) {
        let account_id = env::signer_account_id();
        let pool = self.pool_info.get(usize::try_from(pid).unwrap()).unwrap().clone();
        let transaction = self.user_info.get(&pid).unwrap().get(&account_id).unwrap().clone();

        assert!(pool.quarterly_payout, "quarterlyPayout disabled for pool");
        assert!(pool.pool_type == PoolType::Staking, "poolType not Staking");
        assert!(env::block_timestamp_ms() > pool.deposit_limiters.end_time, "not started");
        
        let mut time_diff = env::block_timestamp_ms() - pool.deposit_limiters.end_time;
        if time_diff > pool.deposit_limiters.duration {
            time_diff = pool.deposit_limiters.duration;
        }

        let quarters_passed = time_diff / QUARTER_DAY;
        assert!(quarters_passed > 0, "too early");
        
        self.transfer_rewards(account_id, pid, index, time_diff, transaction[index].amount);
    }
}

// private and internal
#[near_bindgen]
impl Contract {
    fn _delete_stake_if_empty(&mut self, account_id: AccountId, pid: u128, index: usize) {
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let transaction = self.user_info.entry(pid).or_default().entry(account_id.clone()).or_default();

        if transaction[index].amount == 0 {
            transaction[index] = transaction[transaction.len() - 1].clone();
            transaction.pop();
        }
        if transaction.len() == 0 {
            let is_pool_user = self.is_pool_user.entry(pid).or_default().entry(account_id.clone()).or_default();
            *is_pool_user = false;
            pool.unique_users -= 1;
        }
    }

    fn _calculate_percentage(&self, value: u128, of: u128) -> u128 {
        if of == 0 {
            return 0;
        } else {
            let percentage = value * 100 / of;
            return percentage;
        }
    }

    fn transfer_rewards(&mut self, receiver_id: AccountId, pid: u128, index: usize, duration: u64, amount: u128) -> u128 {
        let reward = self.calculate_interest(receiver_id.clone(), pid, index, amount);
        // let pool = self.pool_info.get(usize::try_from(pid).unwrap()).unwrap().clone();
        let pool = self.pool_info.get_mut(usize::try_from(pid).unwrap()).unwrap();
        let transaction = self.user_info.entry(pid).or_default().entry(receiver_id.clone()).or_default();
        // let transaction = self.user_info.get(&pid).unwrap().get(&account_id).unwrap().clone();
        
        let mut _duration = duration;
        if pool.pool_type == PoolType::Staking {
            if _duration > pool.deposit_limiters.duration {
                _duration = pool.deposit_limiters.duration;
            }
        }
        
        assert!(amount <= transaction[index].amount, "Amount greater than transaction");
        
        let claimable_rewards;
        if reward > transaction[index].paid_out {
            claimable_rewards = reward - transaction[index].paid_out;
        } else {
            claimable_rewards = 0;
        }

        ext_ft::ext(pool.token_info.token.clone())
            .with_static_gas(FT_TRANSFER_GAS)
            .with_attached_deposit(DEPOSIT_ONE_YOCTO)
            .ft_transfer(
                receiver_id.clone().to_string(),
                claimable_rewards.to_string(),
                Some("0".to_string()),
            );

        transaction[index].paid_out += claimable_rewards;

        return claimable_rewards;
    }
}

// view
#[near_bindgen]
impl Contract {
    pub fn total_pools(&self) -> usize {
        return self.pool_info.len();
    }

    pub fn pool_info(&self, pid: usize) -> PoolInfo {
        let mut pool = self.pool_info.get(pid).unwrap().clone();
        ext_ft::ext(pool.token_info.token.clone()).ft_metadata().then(
            Self::ext(env::current_account_id()).ft_metadata_callback(&mut pool)
        );
        return pool;
    }

    pub fn calculate_interest(&self, user: AccountId, pid: u128, index: usize, amount: u128) -> u128 {
        let pool = self.pool_info.get(usize::try_from(pid).unwrap()).unwrap().clone();
        let transaction = self.user_info.get(&pid).unwrap().get(&user).unwrap().clone();

        assert!(amount <= transaction[index].amount, "Amount greater than transaction");

        if pool.pool_type == PoolType::Staking && env::block_timestamp_ms() < pool.deposit_limiters.end_time {
            return 0;
        } else {
            let utilisation: u128;
            if pool.pool_type == PoolType::Loan {
                utilisation = self.get_pool_utilisation(pid);
            } else {
                utilisation = 100;
            }

            let reward_calc_start_time: u64;            
            if pool.pool_type == PoolType::Loan {
                reward_calc_start_time = transaction[index].time;
            } else {
                reward_calc_start_time = pool.deposit_limiters.end_time;
            }

            return amount * pool.apy * utilisation * (env::block_timestamp_ms() as u128 - reward_calc_start_time as u128) / (100 * 100 * 365 * ONE_DAY);
        }
    }

    pub fn get_pool_utilisation(&self, pid: u128) -> u128 {
        let pool = self.pool_info.get(usize::try_from(pid).unwrap()).unwrap().clone();

        if pool.funds.balance == 0 {
            return 0;
        }

        let mut utilisation = self._calculate_percentage(pool.funds.loaned_balance, pool.funds.balance);
        if utilisation > 100 {
            utilisation = 100;
        }

        return utilisation;
    }

    pub fn get_pool_info(&self, from: u128, to: u128) -> Vec<PoolInfo> {
        let mut t_pool_info: Vec<PoolInfo> = Vec::new();
        
        for i in from..to {
            let mut pool = self.pool_info.get(usize::try_from(i).unwrap()).unwrap().clone();
            ext_ft::ext(pool.token_info.token.clone()).ft_metadata().then(
                Self::ext(env::current_account_id()).ft_metadata_callback(&mut pool)
            );
            t_pool_info.push(pool);
        }

        return t_pool_info;
    }

    pub fn total_stakes_of_user(&self, pid: u128, user:AccountId) -> usize {
        return self.user_info.get(&pid).unwrap().get(&user).unwrap().len();
    }

    pub fn get_user_stakes(&self, pid: u128, user: AccountId, from: u128, to: u128) -> Vec<UserInfo> {
        let mut t_user_info: Vec<UserInfo> = Vec::new();

        for i in from..to {
            let per_user_info = self.user_info.get(&pid).unwrap().
            get(&user).unwrap().get(usize::try_from(i).unwrap()).unwrap().clone();
            t_user_info.push(per_user_info);
        }

        return t_user_info;
    }
}

// callback
#[near_bindgen]
impl Contract {
    #[private]
    pub fn ft_metadata_callback(
        &mut self,
        pool_info: &mut PoolInfo,
        #[callback_unwrap] meta: FungibleTokenMetadata,
    ) {
        pool_info.token_info.decimals = meta.decimals;
        pool_info.token_info.name = meta.name;
        pool_info.token_info.symbol = meta.symbol;
    }
}

// modifier
impl Contract {
    fn assert_caller_allowed(&self) {
        if !self.is_owner() {
            env::panic_str("Caller not allowed")
        }
    }

    fn is_owner(&self) -> bool {
        env::signer_account_id() == env::current_account_id()
    }
}

// callback for staking
#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let token_id = env::predecessor_account_id();
        let messages = msg.split(":").map(|x| x.to_string()).collect::<Vec<String>>();
        // assert_eq!(messages.get(0).unwrap(), "staking", "wrong message format");

        let pid = messages[1].trim().parse().expect("should be number");
        let mut result = 0;
        match messages[0].as_str() {
            "staking" => {
                self.internal_deposit_and_stake(sender_id, pid, token_id, amount.0);
                result = 1;
            }
            "borrow" => {
                let index = messages[2].trim().parse().expect("should be number");
                let repay_amount = messages[3].trim().parse().expect("should be number");
                self.internal_repay(sender_id, pid, index, token_id, amount.0, repay_amount);
                result = 2;
            }
            _ => {
                env::panic_str("wrong message format");
            }
        }
        PromiseOrValue::Value(U128(result))
    }
}