use alloy_primitives::U256;

pub trait ProtocolExecutionStrategy {
    fn proof_execution(&self, env: &GuestEvmEnv, total_supply: U256, tally: [U256; 3]) -> bool;
}

mod majority_voting;

pub use majority_voting::MajorityVoting;

use crate::GuestEvmEnv;
