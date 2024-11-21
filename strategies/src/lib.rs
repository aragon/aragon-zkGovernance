pub mod delegation_strategies;
pub mod execution_strategies;
pub mod voting_strategies;

use alloy_primitives::{Address, Bytes, U256};
use anyhow::{bail, Result};
use delegation_strategies::*;
use execution_strategies::*;
use risc0_steel::{Commitment, EvmEnv};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use voting_strategies::*;

pub struct Context {
    voting_power_strategies: HashMap<String, Box<dyn VotingPowerStrategy>>,
    delegation_strategies: HashMap<String, Box<dyn DelegationStrategy>>,
    execution_strategies: HashMap<String, Box<dyn ProtocolExecutionStrategy>>,
    env: EvmEnv<risc0_steel::StateDb, risc0_steel::ethereum::EthBlockHeader, Commitment>,
}

pub(crate) type GuestEvmEnv =
    EvmEnv<risc0_steel::StateDb, risc0_steel::ethereum::EthBlockHeader, Commitment>;

impl Context {
    pub fn new(env: GuestEvmEnv) -> Self {
        Self {
            voting_power_strategies: HashMap::new(),
            delegation_strategies: HashMap::new(),
            execution_strategies: HashMap::new(),
            env,
        }
    }

    pub fn default(env: GuestEvmEnv) -> Self {
        let mut voting_power_strategies: HashMap<String, Box<dyn VotingPowerStrategy>> =
            HashMap::new();
        voting_power_strategies.insert("BalanceOf".to_string(), Box::new(BalanceOf));
        voting_power_strategies.insert("GetPastVotes".to_string(), Box::new(GetPastVotes));

        let mut delegation_strategies: HashMap<String, Box<dyn DelegationStrategy>> =
            HashMap::new();
        delegation_strategies.insert("SplitDelegation".to_string(), Box::new(SplitDelegation));

        let mut execution_strategies: HashMap<String, Box<dyn ProtocolExecutionStrategy>> =
            HashMap::new();
        execution_strategies.insert("MajorityVoting".to_string(), Box::new(MajorityVoting));

        Self {
            voting_power_strategies,
            delegation_strategies,
            execution_strategies,
            env,
        }
    }

    pub fn add_strategy(&mut self, name: String, protocol_strategy: Box<dyn VotingPowerStrategy>) {
        self.voting_power_strategies.insert(name, protocol_strategy);
    }

    pub fn process_voting_strategy(&self, name: String, account: Address, asset: &Asset) -> U256 {
        if let Some(protocol_strategy) = self.voting_power_strategies.get(&name) {
            protocol_strategy.process(&self.env, account, asset)
        } else {
            panic!("Strategy not found: {}", name);
        }
    }

    pub fn process_total_supply(&self, name: String, asset: &Asset) -> U256 {
        if let Some(protocol_strategy) = self.voting_power_strategies.get(&name) {
            protocol_strategy.get_supply(&self.env, asset)
        } else {
            panic!("Strategy not found: {}", name);
        }
    }

    pub fn process_delegation_strategy(
        &self,
        account: Address,
        asset: &Asset,
        additional_data: Bytes,
    ) -> Result<Vec<Delegation>> {
        if let Some(delegation_strategy) = self
            .delegation_strategies
            .get(asset.delegation.strategy.as_str())
        {
            delegation_strategy.process(&self.env, account, asset, additional_data)
        } else {
            bail!("Strategy not found: {}", asset.delegation.strategy);
        }
    }

    pub fn process_execution_strategy(
        &self,
        name: String,
        total_supply: U256,
        tally: [U256; 3],
    ) -> bool {
        if let Some(execution_strategy) = self.execution_strategies.get(&name) {
            execution_strategy.proof_execution(&self.env, total_supply, tally)
        } else {
            panic!("Strategy not found: {}", name);
        }
    }

    pub fn block_commitment(&self) -> Commitment {
        let commitment = self.env.commitment();
        commitment.clone()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationObject {
    pub contract: Address,
    pub strategy: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub contract: Address,
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
