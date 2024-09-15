#!/bin/bash

# Run the setup_deps.sh script to ensure dependencies are set up
#./scripts/setup_deps.sh

# Read the RPC port from temporary file
RPC_PORT=$(cat /tmp/kurtosis_rpc_port)

# Load the .env file and extract the PRIVATE_KEY
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
    PRIVATE_KEY=${PRIVATE_KEY}
else
    echo ".env file not found. Please create a .env file with your PRIVATE_KEY."
    exit 1
fi

if [ -z "$PRIVATE_KEY" ]; then
    echo "PRIVATE_KEY not found in the .env file."
    exit 1
fi

# Run the forge foundry script using the extracted RPC port and PRIVATE_KEY
FORGE_COMMAND="forge script --rpc-url http://127.0.0.1:$RPC_PORT scripts/L2_txn_simulation/ProposeBlock.s.sol -vvvv --broadcast --private-key $PRIVATE_KEY --legacy"

echo "Running forge foundry script..."
eval $FORGE_COMMAND

echo "Forge script execution completed."
