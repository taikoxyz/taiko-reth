#!/bin/bash

# Check if both BLOCKSCOUT_PORT and RUN_LATEST_PATH are provided
if [ -z "$1" ] || [ -z "$2" ]; then
    echo "Error: Both BLOCKSCOUT_PORT and RUN_LATEST_PATH must be provided"
    echo "Usage: $0 <BLOCKSCOUT_PORT> <RUN_LATEST_PATH>"
    exit 1
fi

BLOCKSCOUT_PORT="$1"
RUN_LATEST_PATH="$2"

echo "Using Blockscout port: $BLOCKSCOUT_PORT"
echo "Using run-latest.json path: $RUN_LATEST_PATH"

# Function to verify a regular contract
verify_contract() {
    local address=$1
    local contract_path=$2
    local contract_name=$3

    echo "Verifying contract: $contract_name at address $address"
    forge verify-contract "$address" "$contract_path:$contract_name" \
        --watch --verifier-url "http://localhost:$BLOCKSCOUT_PORT/api" \
        --verifier blockscout --chain-id 160010
}

# Function to verify a proxy contract
verify_proxy_contract() {
    local address=$1
    local arguments=$2

    echo "Verifying proxy contract at address: $address"
    echo "Constructor arguments: $arguments"
    forge verify-contract "$address" "node_modules/@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy" \
        --watch --verifier-url "http://localhost:$BLOCKSCOUT_PORT/api" \
        --verifier blockscout --chain-id 160010 \
        --constructor-args "$arguments" --skip-is-verified-check
}

# Read the run-latest.json file
if [ ! -f "$RUN_LATEST_PATH" ]; then
    echo "Error: run-latest.json not found at $RUN_LATEST_PATH"
    exit 1
fi

RUN_LATEST=$(cat "$RUN_LATEST_PATH")

# Verify regular contracts
verify_all_creates() {
    local contract_name=$1
    local contract_path=$2

    echo "Verifying all instances of $contract_name"
    local addresses=$(jq -r ".transactions[] | select(.contractName == \"$contract_name\" and .transactionType == \"CREATE\") | .contractAddress" <<< "$RUN_LATEST")

    if [ -z "$addresses" ]; then
        echo "No CREATE transactions found for $contract_name"
    else
        echo "$addresses" | while read -r address; do
            if [ ! -z "$address" ]; then
                verify_contract "$address" "$contract_path" "$contract_name"
            fi
        done
    fi
}

verify_all_creates "AddressManager" "contracts/common/AddressManager.sol"
verify_all_creates "TaikoToken" "contracts/tko/TaikoToken.sol"
verify_all_creates "TaikoL1" "contracts/L1/TaikoL1.sol"
verify_all_creates "ChainProver" "contracts/L1/ChainProver.sol"
verify_all_creates "VerifierRegistry" "contracts/L1/VerifierRegistry.sol"
verify_all_creates "MockSgxVerifier" "contracts/L1/verifiers/MockSgxVerifier.sol"

# Verify proxy contracts
echo "Verifying ERC1967Proxy contracts:"
PROXY_CONTRACTS=$(jq -r '.transactions[] | select(.contractName == "ERC1967Proxy" and .transactionType == "CREATE")' <<< "$RUN_LATEST")
echo "$PROXY_CONTRACTS" | jq -c '.' | while read -r proxy; do
    if [ ! -z "$proxy" ]; then
        address=$(echo "$proxy" | jq -r '.contractAddress')
        args=$(echo "$proxy" | jq -r '.arguments | join(",")')
        if [ ! -z "$address" ] && [ ! -z "$args" ]; then
            verify_proxy_contract "$address" "$args"
        else
            echo "Skipping proxy contract due to missing address or arguments"
        fi
    fi
done

echo "All contracts verified."