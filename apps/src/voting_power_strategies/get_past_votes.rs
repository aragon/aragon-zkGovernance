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
        function getPastVotes(address account, uint256 blockNumber) external view returns (uint);
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
        let block_number = env.header().number;
        let mut asset_contract = Contract::preflight(asset.contract, env);
        let balance_call = IERC20Votes::getPastVotesCall {
            account,
            blockNumber: U256::from(block_number),
        };
        let balance = asset_contract
            .call_builder(&balance_call)
            .call()
            .await
            .unwrap();
        U256::from(balance._0)
    }
}
