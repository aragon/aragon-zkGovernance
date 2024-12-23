// Copyright 2024 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// The following library provides utility functions to help with sending
// transactions to a deployed app contract on Ethereum.

pub mod delegation_strategies;
pub mod execution_strategies;
pub mod voting_power_strategies;
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::{Bytes, U256};
use anyhow::{bail, Result};
use delegation_strategies::*;
use execution_strategies::*;
use risc0_steel::{
    ethereum::EthEvmEnv,
    host::{
        db::{AlloyDb, ProofDb},
        HostCommit,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use voting_power_strategies::*;

//type HostEvmEnv<D, H, C> = EvmEnv<ProofDb<D>, H, HostCommit<C>>;
type EthHostEvmEnv<T, N, P, C> = EthEvmEnv<ProofDb<AlloyDb<T, N, P>>, HostCommit<C>>;

/// Wrapper for the commit on the host.

pub struct HostContext<'a, T, N, P, H>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + 'static,
    H: Clone + Send + 'static,
{
    voting_power_strategies: HashMap<String, Box<dyn VotingPowerStrategy<T, N, P, H>>>,
    delegation_strategies: HashMap<String, Box<dyn DelegationStrategy<T, N, P, H>>>,
    execution_strategies: HashMap<String, Box<dyn ExecutionStrategy<T, N, P, H>>>,
    env: &'a mut EthHostEvmEnv<T, N, P, H>,
}

impl<'a, T, N, P, H> HostContext<'a, T, N, P, H>
where
    T: Transport + Clone,
    T: Transport + Clone + Send + Sync,
    N: Network + Send + Sync,
    P: Provider<T, N> + Send + Sync + 'static,
    H: Clone + Send + Sync + 'static,
{
    pub fn default(env: &'a mut EthHostEvmEnv<T, N, P, H>) -> Self {
        let mut voting_power_strategies: HashMap<String, Box<dyn VotingPowerStrategy<T, N, P, H>>> =
            HashMap::new();
        voting_power_strategies.insert("BalanceOf".to_string(), Box::new(BalanceOf));
        voting_power_strategies.insert("GetPastVotes".to_string(), Box::new(GetPastVotes));

        let mut delegation_strategies: HashMap<String, Box<dyn DelegationStrategy<T, N, P, H>>> =
            HashMap::new();
        delegation_strategies.insert("SplitDelegation".to_string(), Box::new(SplitDelegation));

        let mut execution_strategies: HashMap<String, Box<dyn ExecutionStrategy<T, N, P, H>>> =
            HashMap::new();
        execution_strategies.insert("MajorityVoting".to_string(), Box::new(MajorityVoting));

        Self {
            voting_power_strategies,
            delegation_strategies,
            execution_strategies,
            env,
        }
    }

    pub async fn process_voting_power_strategy(
        &mut self,
        name: String,
        account: alloy_primitives::Address,
        asset: &Asset,
    ) -> alloy_primitives::U256 {
        if let Some(voting_power_strategy) = self.voting_power_strategies.get(&name) {
            voting_power_strategy
                .process(&mut self.env, account, asset)
                .await
        } else {
            panic!("Strategy not found: {}", name);
        }
    }

    pub async fn process_delegation_strategy(
        &mut self,
        account: alloy_primitives::Address,
        asset: &Asset,
        additional_data: Bytes,
    ) -> Result<Vec<Delegation>> {
        if let Some(delegation_strategy) = self
            .delegation_strategies
            .get(asset.delegation.strategy.as_str())
        {
            delegation_strategy
                .process(&mut self.env, account, asset, additional_data)
                .await
        } else {
            panic!("Strategy not found: {}", asset.delegation.strategy);
        }
    }

    pub async fn process_total_supply(&mut self, asset: &Asset) -> Result<U256> {
        if let Some(voting_strategy) = self
            .voting_power_strategies
            .get(&asset.voting_power_strategy)
        {
            Ok(voting_strategy.get_supply(&mut self.env, asset).await)
        } else {
            bail!("Strategy not found: {}", &asset.voting_power_strategy);
        }
    }

    pub async fn process_execution_strategy(
        &mut self,
        name: String,
        total_supply: U256,
        tally: [U256; 3],
    ) -> Result<bool> {
        if let Some(execution_strategy) = self.execution_strategies.get(&name) {
            Ok(execution_strategy
                .proof_execution(&mut self.env, total_supply, tally)
                .await)
        } else {
            bail!("Strategy not found: {}", name);
        }
    }
}

// The input of the config
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationObject {
    pub contract: alloy_primitives::Address,
    pub strategy: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub contract: alloy_primitives::Address,
    pub chain_id: u64,
    pub voting_power_strategy: String,
    pub delegation: DelegationObject,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiscVotingProtocolConfig {
    pub voting_protocol_version: String,
    pub assets: Vec<Asset>,
    pub execution_strategy: String,
}
