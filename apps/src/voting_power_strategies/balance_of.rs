use super::VotingPowerStrategy;
use crate::{Asset, EthHostEvmEnv};
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use async_trait::async_trait;
use risc0_steel::Contract;

sol! {
    /// ERC-20 balance function signature.
    interface IERC20 {
        function balanceOf(address account) external view returns (uint);
        function getTotalSupply() external view returns (uint);
    }
}

pub struct BalanceOf;

#[async_trait]
impl<T, N, P, H> VotingPowerStrategy<T, N, P, H> for BalanceOf
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + 'static,
    H: Send + 'static,
{
    async fn process(
        &self,
        env: &mut EthHostEvmEnv<T, N, P, H>,
        account: Address,
        asset: &Asset,
    ) -> U256 {
        let mut asset_contract = Contract::preflight(asset.contract, env);
        let balance_call = IERC20::balanceOfCall { account };
        let balance = asset_contract
            .call_builder(&balance_call)
            .call()
            .await
            .unwrap();
        U256::from(balance._0)
    }
}
