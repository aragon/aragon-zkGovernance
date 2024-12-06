use super::VotingPowerStrategy;
use crate::{Asset, EthHostEvmEnv};
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use async_trait::async_trait;
use risc0_steel::Contract;

sol! {
    /// ERC-20 balance function signature.
    interface IERC20Votes {
        function getPastVotes(address account, uint256 timepoint) public view returns (uint256);
        function getPastTotalSupply(uint256 timepoint) external view returns (uint);
    }
}

pub struct GetPastVotes;

#[async_trait]
impl<T, N, P, H> VotingPowerStrategy<T, N, P, H> for GetPastVotes
where
    T: Transport + Clone,
    N: Network,
    // P: Provider + revm::primitives::db::Database,
    P: Provider<T, N> + Send + 'static,
    H: Send + 'static,
{
    async fn process(
        &self,
        env: &mut EthHostEvmEnv<T, N, P, H>,
        account: Address,
        asset: &Asset,
    ) -> U256 {
        let block_number = env.header().parent_num_hash().number;
        let mut asset_contract = Contract::preflight(asset.contract, env);
        let past_votes_call = IERC20Votes::getPastVotesCall {
            account,
            timepoint: U256::from(block_number),
        };
        let past_votes = asset_contract
            .call_builder(&past_votes_call)
            .call()
            .await
            .unwrap();
        U256::from(past_votes._0)
    }
}

// Unit tests module
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::str::FromStr;

    use alloy::transports::http::reqwest::Url;
    use alloy_primitives::address;
    use risc0_steel::ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC};

    use crate::DelegationObject;

    use super::*;

    #[tokio::test]
    async fn test_process() -> Result<()> {
        let mut env = EthEvmEnv::builder()
            .rpc(Url::from_str(&std::env::var("RPC_URL").unwrap()).unwrap())
            .build()
            .await
            .unwrap();
        env = env.with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

        let account = address!("8bF1e340055c7dE62F11229A149d3A1918de3d74");
        let asset: Asset = Asset {
            contract: address!("185Bb1cca668C474214e934028A3e4BB7A5E6525"),
            chain_id: ETH_SEPOLIA_CHAIN_SPEC.chain_id(),
            voting_power_strategy: "GetPastVotes".to_string(),
            delegation: DelegationObject {
                contract: address!("185Bb1cca668C474214e934028A3e4BB7A5E6525"),
                strategy: "SplitDelegation".to_string(),
            },
        };
        let past_votes_strategy = GetPastVotes;
        let past_votes = past_votes_strategy.process(&mut env, account, &asset).await;
        assert_eq!(past_votes, U256::from_str("900000000000000000").unwrap());
        Ok(())
    }
}
