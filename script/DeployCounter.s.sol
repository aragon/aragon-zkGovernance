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
//
// SPDX-License-Identifier: Apache-2.0

pragma solidity ^0.8.17;

import {IRiscZeroVerifier} from "risc0/IRiscZeroVerifier.sol";
import {ControlID, RiscZeroGroth16Verifier} from "risc0/groth16/RiscZeroGroth16Verifier.sol";

import {MajorityVotingBase} from "../contracts/MajorityVotingBase.sol";
import {RiscVotingProtocolPlugin} from "../contracts/RiscVotingProtocolPlugin.sol";
import {RiscVotingProtocolPluginSetup} from "../contracts/RiscVotingProtocolPluginSetup.sol";
import {ERC20} from "../contracts/ERC20.sol";

import {Script, console2} from "forge-std/Script.sol";
import {Vm} from "forge-std/Test.sol";

import {PluginRepoFactory} from "@aragon/osx/framework/plugin/repo/PluginRepoFactory.sol";
import {PluginRepo} from "@aragon/osx/framework/plugin/repo/PluginRepo.sol";
import {DAOFactory} from "@aragon/osx/framework/dao/DAOFactory.sol";
import {hashHelpers, PluginSetupRef} from "@aragon/osx/framework/plugin/setup/PluginSetupProcessorHelpers.sol";

/// @notice Deployment script for the Counter contract.
/// @dev Use the following environment variable to control the deployment:
///     * ETH_WALLET_PRIVATE_KEY private key of the wallet to be used for deployment.
///
/// See the Foundry documentation for more information about Solidity scripts.
/// https://book.getfoundry.sh/tutorials/solidity-scripting
contract Deploy is Script {
    address pluginRepoFactory;
    DAOFactory daoFactory;
    string nameWithEntropy;
    address[] pluginAddress;
    string votingProtocolConfig;
    address token;

    function setUp() public {
        pluginRepoFactory = vm.envAddress("PLUGIN_REPO_FACTORY");
        daoFactory = DAOFactory(vm.envAddress("DAO_FACTORY"));
        nameWithEntropy = string.concat(
            "risc-voting-protocol-",
            vm.toString(block.timestamp)
        );
        votingProtocolConfig = vm.readFile(
            "script/RiscVotingProtocolConfig.txt"
        );
        token = vm.envAddress("TOKEN");
    }

    function run() external {
        // 0. Setting up Foundry
        vm.startBroadcast(vm.envUint("ETH_WALLET_PRIVATE_KEY"));

        // 1. Deploying the Plugin Setup
        RiscVotingProtocolPluginSetup pluginSetup = deployPluginSetup();

        // 2. Publishing it in the Aragon OSx Protocol
        PluginRepo pluginRepo = deployPluginRepo(address(pluginSetup));

        // 3. Defining the DAO Settings
        DAOFactory.DAOSettings memory daoSettings = getDAOSettings();

        // 4. Defining the plugin settings
        DAOFactory.PluginSettings[] memory pluginSettings = getPluginSettings(
            pluginRepo
        );

        // 5. Deploying the DAO
        vm.recordLogs();
        address createdDAO = address(
            daoFactory.createDao(daoSettings, pluginSettings)
        );

        // 6. Getting the Plugin Address
        Vm.Log[] memory logEntries = vm.getRecordedLogs();

        for (uint256 i = 0; i < logEntries.length; i++) {
            if (
                logEntries[i].topics[0] ==
                keccak256(
                    "InstallationApplied(address,address,bytes32,bytes32)"
                )
            ) {
                pluginAddress.push(
                    address(uint160(uint256(logEntries[i].topics[2])))
                );
            }
        }

        vm.stopBroadcast();

        // 7. Logging the resulting addresses
        console2.log("Plugin Setup: ", address(pluginSetup));
        console2.log("Plugin Repo: ", address(pluginRepo));
        console2.log("Created DAO: ", address(createdDAO));
        console2.log("Installed Plugins: ");
        for (uint256 i = 0; i < pluginAddress.length; i++) {
            console2.log("- ", pluginAddress[i]);
        }
    }

    function deployPluginSetup()
        public
        returns (RiscVotingProtocolPluginSetup)
    {
        RiscVotingProtocolPluginSetup pluginSetup = new RiscVotingProtocolPluginSetup();
        return pluginSetup;
    }

    function deployPluginRepo(
        address pluginSetup
    ) public returns (PluginRepo pluginRepo) {
        pluginRepo = PluginRepoFactory(pluginRepoFactory)
            .createPluginRepoWithFirstVersion(
                nameWithEntropy,
                pluginSetup,
                msg.sender,
                "0x00", // TODO: Give these actual values on prod
                "0x00"
            );
    }

    function getDAOSettings()
        public
        view
        returns (DAOFactory.DAOSettings memory)
    {
        return DAOFactory.DAOSettings(address(0), "", nameWithEntropy, "");
    }

    function getPluginSettings(
        PluginRepo pluginRepo
    ) public view returns (DAOFactory.PluginSettings[] memory pluginSettings) {
        // TODO: Get the members from a json file
        address[] memory members = new address[](1);
        members[0] = address(msg.sender);
        MajorityVotingBase.VotingSettings
            memory votingSettings = MajorityVotingBase.VotingSettings({
                votingMode: MajorityVotingBase.VotingMode.Standard,
                supportThreshold: 20,
                minParticipation: 10,
                minDuration: 0.1 days,
                minProposerVotingPower: 1,
                votingProtocolConfig: votingProtocolConfig
            });

        IRiscZeroVerifier verifier = IRiscZeroVerifier(
            0x925d8331ddc0a1F0d96E68CF073DFE1d92b69187
        );
        bytes memory pluginSettingsData = abi.encode(
            votingSettings,
            token,
            verifier
        );

        PluginRepo.Tag memory tag = PluginRepo.Tag(1, 1);
        pluginSettings = new DAOFactory.PluginSettings[](1);
        pluginSettings[0] = DAOFactory.PluginSettings(
            PluginSetupRef(tag, pluginRepo),
            pluginSettingsData
        );
    }
}
