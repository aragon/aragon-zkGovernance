# Build the project
echo "Building the project..."
cargo build

# These are some examples of live values that you can use to test the publisher
export TOYKEN_ADDRESS=0x185Bb1cca668C474214e934028A3e4BB7A5E6525
export PROVING_BLOCK_NUMBER=7087022
export VOTER_SIGNATURE="476d1ca40c07dad98cad9acf0b08e673ae0ea7b2efb01453bd123333c800c6ee3b6f2a642e7aa03e777cfea4fec2c790ca99515a2d4acad0adbf062602ef0b9d1c"
export VOTER=0x8bF1e340055c7dE62F11229A149d3A1918de3d74
#export VOTER=0x8bF1e340055c7dE62F11229A149d3A1918de3d74
export COUNTER_ADDRESS=0xaf4ba5015Eb5bE8780e664e2BE40144668361B0f
export DAO_ADDRESS=0xB32806A45fDdB87747bb641A890D10F2F819c267
export PROPOSAL_ID=0
export DIRECTION=2
export BALANCE=450000000000000000
export ADDITIONAL_DELEGATION_DATA="8bF1e340055c7dE62F11229A149d3A1918de3d74"

COUNTER_VALUE=$(cast call --rpc-url ${RPC_URL} ${COUNTER_ADDRESS:?} 'get()(uint256)')

echo ""
echo "----------------------------------------------------------------------"
echo "|                                                                     |"
echo "|  You should have exported you testnet private key for this to work  |"
echo "|  You should have exported you testnet RPC_URL for this to work      |"
echo "|                                                                     |"
echo "----------------------------------------------------------------------"
echo ""
echo "ERC20 Toyken Address: $TOYKEN_ADDRESS"
echo "Initial block number: $PROVING_BLOCK_NUMBER"
echo "Counter Address: $COUNTER_ADDRESS"
echo "Address: $USER_ADDRESS"
echo "Counter value: $COUNTER_VALUE"

# Publish a new state
echo "Publishing a new state..."
cargo run --bin publisher -- \
    --chain-id=11155111 \
    --rpc-url=${RPC_URL} \
    --block-number=${PROVING_BLOCK_NUMBER:?} \
    --voter-signature=${VOTER_SIGNATURE} \
    --voter=${VOTER} \
    --dao-address=${DAO_ADDRESS} \
    --proposal-id=${PROPOSAL_ID} \
    --direction=${DIRECTION} \
    --balance=${BALANCE} \
    --config-contract=${COUNTER_ADDRESS:?} \
    --token=${TOYKEN_ADDRESS:?} \
    --additional-delegation-data=${ADDITIONAL_DELEGATION_DATA:?}

# Attempt to verify counter value as part of the script logic
echo "All operations completed successfully."
