use super::VotingPowerStrategy;
use crate::{Asset, GuestEvmEnv};
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use risc0_steel::Contract;

sol! {
    /// ERC-20 balance function signature.
    interface IERC20Votes {
        function getPastVotes(address account, uint256 blockNumber) external view returns (uint);
        function getPastTotalSupply(uint256 timepoint) external view returns (uint);
    }
}

pub struct GetPastVotes;
impl VotingPowerStrategy for GetPastVotes {
    fn process(&self, env: &GuestEvmEnv, account: Address, asset: &Asset) -> U256 {
        let block_number = env.header().number;
        let asset_contract = Contract::new(asset.contract, env);
        let balance_call = IERC20Votes::getPastVotesCall {
            account,
            blockNumber: U256::from(block_number),
        };
        let balance = asset_contract.call_builder(&balance_call).call();
        U256::from(balance._0)
    }

    fn get_supply(&self, env: &GuestEvmEnv, asset: &Asset) -> U256 {
        let block_number = env.header().number;
        let asset_contract = Contract::new(asset.contract, env);
        let supply_call = IERC20Votes::getPastTotalSupplyCall {
            timepoint: U256::from(block_number),
        };
        let supply = asset_contract.call_builder(&supply_call).call();
        U256::from(supply._0)
    }
}
