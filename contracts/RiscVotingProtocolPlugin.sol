// SPDX-License-Identifier: AGPL-3.0-or-later

pragma solidity ^0.8.17;

import {MajorityVotingBase} from "./MajorityVotingBase.sol";

import {IERC20Upgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC20/IERC20Upgradeable.sol";
import {SafeCastUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/math/SafeCastUpgradeable.sol";

import {IMembership} from "@aragon/osx/core/plugin/membership/IMembership.sol";
import {IDAO} from "@aragon/osx/core/dao/IDAO.sol";

import {IRiscZeroVerifier} from "risc0/IRiscZeroVerifier.sol";
import {Steel, Encoding} from "risc0/steel/Steel.sol";
import {ImageID} from "./ImageID.sol"; // auto-generated contract after running `cargo build`.

/// @title Counter
/// @notice Implements a counter that increments based on off-chain Steel proofs submitted to this contract.
/// @dev The contract interacts with ERC-20 tokens, using Steel proofs to verify that an account holds at least 1 token
/// before incrementing the counter. This contract leverages RISC0-zkVM for generating and verifying these proofs.
contract RiscVotingProtocolPlugin is MajorityVotingBase {
    using SafeCastUpgradeable for uint256;

    /// @notice Journal that is committed to by the guest.
    struct VotingJournal {
        Steel.Commitment commitment;
        address configContract;
        uint256 proposalId;
        address voter;
        uint256 balance;
        uint8 direction;
    }

    // TODO: Journal should be in sync with guest
    struct ExecutionJournal {
        Steel.Commitment commitment;
        address configContract;
        uint256 proposalId;
        address voter;
        uint256 balance;
        uint8 direction;
    }

    /// @notice Counter to track the number of successful verifications.
    uint256 public counter;

    function initialize(
        IDAO _dao,
        VotingSettings calldata _votingSettings
    ) external initializer {
        // _votingSettings.votingProtocolImageId = ImageID.VOTING_PROTOCOL_ID;
        // _votingSettings.executionProtocolImageId = ImageID.EXECUTION_PROTOCOL_ID;
        __MajorityVotingBase_init(_dao, _votingSettings);
    }

    /// @inheritdoc MajorityVotingBase
    function createProposal(
        bytes calldata _metadata,
        IDAO.Action[] calldata _actions,
        uint256 _allowFailureMap,
        uint64 _startDate,
        uint64 _endDate
    )
        external
        override
        auth(CREATE_PROPOSAL_PERMISSION_ID)
        returns (uint256 proposalId)
    {
        uint256 snapshotBlock;
        unchecked {
            // The snapshot block must be mined already to
            // protect the transaction against backrunning transactions causing census changes.
            snapshotBlock = block.number - 1;
        }

        (_startDate, _endDate) = _validateProposalDates(_startDate, _endDate);

        proposalId = _createProposal({
            _creator: _msgSender(),
            _metadata: _metadata,
            _startDate: _startDate,
            _endDate: _endDate,
            _actions: _actions,
            _allowFailureMap: _allowFailureMap
        });

        // Store proposal related information
        Proposal storage proposal_ = proposals[proposalId];

        proposal_.parameters.startDate = _startDate;
        proposal_.parameters.endDate = _endDate;
        proposal_.parameters.snapshotBlock = snapshotBlock.toUint64();
        proposal_.parameters.votingMode = votingMode();
        proposal_.parameters.supportThreshold = supportThreshold();
        proposal_.parameters.snapshotBlockHash = blockhash(snapshotBlock);
        proposal_.parameters.votingProtocolConfig = votingProtocolConfig();
        proposal_.parameters.verifier = verifier();
        proposal_.parameters.votingProtocolImageId = votingProtocolImageId();
        proposal_
            .parameters
            .executionProtocolImageId = executionProtocolImageId();

        // Reduce costs
        if (_allowFailureMap != 0) {
            proposal_.allowFailureMap = _allowFailureMap;
        }

        for (uint256 i; i < _actions.length; ) {
            proposal_.actions.push(_actions[i]);
            unchecked {
                ++i;
            }
        }
    }

    function vote(
        bytes calldata journalData,
        bytes calldata seal
    ) external override {
        // Decode and validate the journal data
        VotingJournal memory journal = abi.decode(journalData, (VotingJournal));
        require(
            journal.configContract == address(this),
            "Invalid token address"
        );

        Proposal storage proposal_ = proposals[journal.proposalId];

        require(
            journal.commitment.digest == proposal_.parameters.snapshotBlockHash,
            "Invalid commitment"
        );

        (uint240 claimID, uint16 _version) = Encoding.decodeVersionedID(
            journal.commitment.id
        );
        require(
            claimID == proposal_.parameters.snapshotBlock,
            "Invalid commitment"
        );

        // Verify the proof
        bytes32 journalHash = sha256(journalData);
        IRiscZeroVerifier(proposal_.parameters.verifier).verify(
            seal,
            proposal_.parameters.votingProtocolImageId,
            journalHash
        );

        // The actual vote
        // This could re-enter, though we can assume the governance token is not malicious
        uint256 votingPower = journal.balance;
        address _voter = journal.voter;
        VoteOption state = proposal_.voters[_voter];

        // If voter had previously voted, decrease count
        if (state == VoteOption.Yes) {
            proposal_.tally.yes = proposal_.tally.yes - votingPower;
        } else if (state == VoteOption.No) {
            proposal_.tally.no = proposal_.tally.no - votingPower;
        } else if (state == VoteOption.Abstain) {
            proposal_.tally.abstain = proposal_.tally.abstain - votingPower;
        }

        // write the updated/new vote for the voter.
        VoteOption _voteOption = VoteOption(journal.direction);
        if (_voteOption == VoteOption.Yes) {
            proposal_.tally.yes = proposal_.tally.yes + votingPower;
        } else if (_voteOption == VoteOption.No) {
            proposal_.tally.no = proposal_.tally.no + votingPower;
        } else if (_voteOption == VoteOption.Abstain) {
            proposal_.tally.abstain = proposal_.tally.abstain + votingPower;
        }

        proposal_.voters[_voter] = _voteOption;

        emit VoteCast({
            proposalId: journal.proposalId,
            voter: _voter,
            voteOption: _voteOption,
            votingPower: votingPower
        });
    }

    /// @inheritdoc MajorityVotingBase
    function _canVote(
        uint256 _proposalId,
        address _account,
        VoteOption _voteOption
    ) internal view override returns (bool) {
        Proposal storage proposal_ = proposals[_proposalId];

        // The proposal vote hasn't started or has already ended.
        if (!_isProposalOpen(proposal_)) {
            return false;
        }

        // The voter votes `None` which is not allowed.
        if (_voteOption == VoteOption.None) {
            return false;
        }

        // The voter has already voted but vote replacment is not allowed.
        if (
            proposal_.voters[_account] != VoteOption.None &&
            proposal_.parameters.votingMode != VotingMode.VoteReplacement
        ) {
            return false;
        }

        return true;
    }

    /// @inheritdoc MajorityVotingBase
    function execute(
        bytes calldata journalData,
        bytes calldata seal
    ) public override {
        ExecutionJournal memory executionJournal = abi.decode(
            journalData,
            (ExecutionJournal)
        );
        require(
            executionJournal.configContract == address(this),
            "Invalid token address"
        );
        uint256 _proposalId = executionJournal.proposalId;
        Proposal storage proposal_ = proposals[_proposalId];

        if (!_canExecute(_proposalId)) {
            revert ProposalExecutionForbidden(_proposalId);
        }

        require(
            executionJournal.commitment.digest ==
                proposal_.parameters.snapshotBlockHash,
            "Invalid commitment"
        );
        (uint240 claimID, uint16 _version) = Encoding.decodeVersionedID(
            executionJournal.commitment.id
        );
        require(
            claimID == proposal_.parameters.snapshotBlock,
            "Invalid commitment"
        );

        // Verify the proof
        bytes32 journalHash = sha256(journalData);
        IRiscZeroVerifier(proposal_.parameters.verifier).verify(
            seal,
            proposal_.parameters.executionProtocolImageId,
            journalHash
        );

        _execute(_proposalId);
    }

    // TODO: Revisit this number
    /// @dev This empty reserved space is put in place to allow future versions to add new
    /// variables without shifting down storage in the inheritance chain.
    /// https://docs.openzeppelin.com/contracts/4.x/upgradeable#storage_gaps
    uint256[49] private __gap;
}
