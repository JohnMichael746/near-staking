use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::u128;
use near_sdk::{env, near_bindgen, AccountId, PanicOnDefault, PromiseOrValue};

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    decimals: u8,
    name: String,
    symbol: String,
    icon: Option<String>,
    max_mint: Option<u128>,
    minter: Option<AccountId>,
}

near_contract_standards::impl_fungible_token_core!(Contract, token);
near_contract_standards::impl_fungible_token_storage!(Contract, token);

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(
        decimals: u8,
        name: String,
        symbol: String,
        icon: Option<String>,
        max_mint: Option<u128>,
        minter: Option<AccountId>,
    ) -> Self {
        Self {
            token: FungibleToken::new(b"t".to_vec()),
            decimals,
            name,
            symbol,
            icon,
            max_mint,
            minter,
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.assert_caller_allowed();
        self.name = name
    }

    pub fn set_icon(&mut self, icon: Option<String>) {
        self.assert_caller_allowed();
        self.icon = icon
    }

    pub fn set_symbol(&mut self, symbol: String) {
        self.assert_caller_allowed();
        self.symbol = symbol
    }

    pub fn set_max_mint(&mut self, max_mint: Option<u128>) {
        self.assert_caller_allowed();
        self.max_mint = max_mint;
    }

    pub fn set_minter(&mut self, minter: Option<AccountId>) {
        self.assert_caller_allowed();
        self.minter = minter;
    }

    /// Naming this ft_* allows the NEAR wallet to discover this token for you
    #[payable]
    pub fn ft_mint(&mut self, receiver_id: AccountId, amount: u128) {
        if let Some(max_mint) = self.max_mint {
            let amount: u128 = amount.into();
            if amount > max_mint.into() {
                env::panic_str("Mint amount exceeds maximum");
            }
        }
        if self.is_owner_or_minter() {
            self.token.internal_register_account(&receiver_id);
            self.token.internal_deposit(&receiver_id, amount.into());
        } else {
            env::panic_str("admin or minter only!");
        }
    }

    #[payable]
    pub fn ft_burn(&mut self, account_id: AccountId, amount: u128) {
        if self.is_owner_or_minter() {
            self.token.internal_withdraw(&account_id, amount.into());
        } else {
            env::panic_str("admin or minter only!");
        }
    }

    pub fn unregister_account(&mut self, account_id: &AccountId) {
        if self.is_owner_or_minter() {
            if self.token.accounts.remove(account_id).is_none() {
                env::panic_str("The account does not exist");
            }
        } else {
            env::panic_str("admin or minter only!");
        }
    }

    fn ft_transfer(&mut self, receiver_id: AccountId, amount: u128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    fn ft_transfer_from(&mut self, sender_id: AccountId, receiver_id: AccountId, amount: u128, memo: Option<String>) {
        if self.is_owner_or_minter() {
            self.token.internal_transfer(&sender_id, &receiver_id, amount.into(), memo);
        } else {
            env::panic_str("admin or minter only!");
        }
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: u128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<u128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }
}

impl Contract {
    fn assert_caller_allowed(&self) {
        if !self.is_owner_or_minter() {
            env::panic_str("Caller not allowed")
        }
    }

    fn is_owner_or_minter(&self) -> bool {
        if let Some(minter1) = self.minter.clone() {
            return env::signer_account_id() == env::current_account_id() || env::signer_account_id() == minter1
        }
        return false;
    }
}

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            reference: None,
            reference_hash: None,
            decimals: self.decimals,
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            icon: self.icon.clone(),
        }
    }
}