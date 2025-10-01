// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/* ABI of the Fungible Token Example Application */

pub use linera_sdk::abis::fungible::*;
use linera_sdk::linera_base_types::{Account, AccountOwner, Amount};
use linera_sdk::abi::{ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};
#[cfg(all(any(test, feature = "test"), not(target_arch = "wasm32")))]
use {
    futures::{stream, StreamExt},
    linera_sdk::{
        linera_base_types::{ApplicationId, ModuleId},
        test::{ActiveChain, QueryOutcome, TestValidator},
    },
};

pub struct FungibleNoGraphQlTokenAbi;

impl ContractAbi for FungibleNoGraphQlTokenAbi {
    type Operation = FungibleOperation;
    type Response = FungibleResponse;
}

impl ServiceAbi for FungibleNoGraphQlTokenAbi {
    type Query = FungibleNoGraphQlRequest;
    type QueryResponse = Amount;
}

#[derive(Debug, Deserialize, Serialize)]
pub enum FungibleNoGraphQlRequest {
    Balance {
        /// Owner to query the balance for
        owner: AccountOwner,
    },
    Allowance {
        /// Owner of the balance for the allowance
        owner: AccountOwner,
        /// Spender for the allowance in question.
        spender: AccountOwner,
    },
    Operation {
        /// The operation in question.
        operation: FungibleOperation,
    },
    Operations {
        /// The operation in question.
        operations: Vec<FungibleOperation>,
    },
}




/// A message.
#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    /// Credits the given `target` account, unless the message is bouncing, in which case
    /// `source` is credited instead.
    Credit {
        /// Target account to credit amount to
        target: AccountOwner,
        /// Amount to be credited
        amount: Amount,
        /// Source account to remove amount from
        source: AccountOwner,
    },

    /// Withdraws from the given account and starts a transfer to the target account.
    Withdraw {
        /// Account to withdraw from
        owner: AccountOwner,
        /// Amount to be withdrawn
        amount: Amount,
        /// Target account to transfer amount to
        target_account: Account,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OwnerSpender {
    /// Account to withdraw from
    pub owner: AccountOwner,
    /// Account to do the withdrawing
    pub spender: AccountOwner,
}

impl OwnerSpender {
    pub fn new(owner: AccountOwner, spender: AccountOwner) -> Self {
        if owner == spender {
            panic!("owner should be different from spender");
        }
        Self { owner, spender }
    }
}

