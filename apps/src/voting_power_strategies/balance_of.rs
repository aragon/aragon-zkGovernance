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

    async fn get_supply(&self, env: &mut EthHostEvmEnv<T, N, P, H>, asset: &Asset) -> U256 {
        let mut asset_contract = Contract::preflight(asset.contract, env);
        let supply_call = IERC20::getTotalSupplyCall {};
        let supply = asset_contract
            .call_builder(&supply_call)
            .call()
            .await
            .unwrap();
        U256::from(supply._0)
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
            voting_power_strategy: "BalanceOf".to_string(),
            delegation: DelegationObject {
                contract: address!("185Bb1cca668C474214e934028A3e4BB7A5E6525"),
                strategy: "SplitDelegation".to_string(),
            },
        };
        let balance_strategy = BalanceOf;
        let balance = balance_strategy.process(&mut env, account, &asset).await;
        assert_eq!(balance, U256::from_str("900000000000000000").unwrap());
        Ok(())
    }
}
