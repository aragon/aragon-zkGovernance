# zkVoting: Aragon OSx Plugin for RISC Zero's zkVM proofs verification

This Aragon OSx plugin introduces a zk proof verification system that redefines how voting, delegation, and execution are managed in DAOs. Powered by RISC Zero’s zkVM, the system enables off-chain proof generation and on-chain verification, ensuring secure, scalable, and gas-efficient governance.

The plugin includes two main components:
	1.	OSx Plugin Contracts – Integrate seamlessly into Aragon’s governance framework.
	2.	zkVM Proof Programs – Designed as a flexible voting protocol, these programs allow DAOs to define custom voting, delegation, and execution strategies.

When installing the plugin, DAOs configure their governance rules and strategies through a JSON string. This allows them to manage multiple assets, each with unique delegation mechanisms tailored to their needs.

Unique Features:

	•	Token Flexibility: Enables tracking balances and delegation for tokens that don’t support ERC20Votes, unlocking new asset compatibility for governance.
	•	Gas Efficiency: Acts as an additional execution layer to reduce gas costs for high-demand governance contracts.
	•	Customizable Strategies: Build bespoke governance processes, from voting to execution, directly within the zk proof framework.

With multichain support planned for future updates, this plugin equips DAOs with a powerful, future-proof governance solution that combines flexibility, security, and efficiency.

## Dependencies

First, [install Rust] and [Foundry], and then restart your terminal.

```sh
# Install Rust
curl https://sh.rustup.rs -sSf | sh
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash
```

Next, you will need to install the `cargo risczero` tool.
We'll use [`cargo binstall`][cargo-binstall] to get `cargo-risczero` installed, and then install the `risc0` toolchain.
See [RISC Zero installation] for more details.

```sh
cargo install cargo-binstall
cargo binstall cargo-risczero
cargo risczero install
```

Now you have all the tools you need to develop and deploy an application with [RISC Zero].

### Configuring Bonsai

***Note:*** *To request an API key [complete the form here](https://bonsai.xyz/apply).*

With the Bonsai proving service, you can produce a [Groth16 SNARK proof] that is verifiable on-chain.
You can get started by setting the following environment variables with your API key and associated URL.

```bash
export BONSAI_API_KEY="YOUR_API_KEY" # see form linked above
export BONSAI_API_URL="BONSAI_URL" # provided with your api key
```

## Deploy Your Plugin

First you'll need to compile a release version:

```bash
cargo build --release --bin publisher
```

Then, there'll be some scaffolding on the proving programs and the contracts. Due to a current limitation, the contracts generated can't be used directly.
The reason is solidity uncompatibility of versions.

Go to the files:
- contracts/ImageID.sol
- contgract/Elf.sol

And change their solidity versions to: `pragma solidity ^0.8.17;`. That is, change in both contracts the version from solidity 20 to 17.

After that, you'll now be able to deploy the contracts:
```bash
export ETH_WALLET_PRIVATE_KEY=<<YOUR_TESTING_PK>>
forge script --rpc-url  https://eth-sepolia.g.alchemy.com/v2/<<YOUR_ALCHEMY_KEY>> script/DeployCounter.s.sol:Deploy --broadcast --verify --etherscan-api-key  <YOUR_ETHERSCAN_KEY>
```

Once you've done that, you'll have a DAO deployed with the OSx Plugin.
