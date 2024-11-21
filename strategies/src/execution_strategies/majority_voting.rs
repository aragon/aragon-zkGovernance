use crate::GuestEvmEnv;

use super::ProtocolExecutionStrategy;
use alloy_primitives::U256;

pub struct MajorityVoting;
impl ProtocolExecutionStrategy for MajorityVoting {
    fn proof_execution(&self, _env: &GuestEvmEnv, total_supply: U256, tally: [U256; 3]) -> bool {
        // TODO: The parameters for the minimum partticipation and so on should be flexible
        let yes_votes = tally[0];
        let no_votes = tally[1];
        let abstain_votes = tally[2];

        // Calculate the total votes cast
        let total_votes = yes_votes + no_votes + abstain_votes;

        // Check if the total votes cast is more than 50% of the total supply
        if total_votes <= total_supply / U256::from(2) {
            return false; // Not enough participation
        }

        // Calculate the threshold for passing (more than 50% of non-abstain votes)
        let non_abstain_votes = yes_votes + no_votes;
        let threshold = non_abstain_votes / U256::from(2);

        // The proposal passes if yes votes are greater than the threshold
        yes_votes > threshold
    }
}
