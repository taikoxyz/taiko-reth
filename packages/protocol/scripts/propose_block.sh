#!/bin/bash

# Run the setup_deps.sh script to ensure dependencies are set up
#./scripts/setup_deps.sh

# Check if a number parameter was provided
if [ -z "$1" ]; then
    echo "Error: No number parameter provided. Usage: ./propose_block.sh <number>"
    exit 1
fi

# Construct the script filename
SCRIPT_FILE="ProposeBlock$1.s.sol"

# Check if the constructed filename exists
if [ ! -f "scripts/L2_txn_simulation/$SCRIPT_FILE" ]; then
    echo "Error: File scripts/L2_txn_simulation/$SCRIPT_FILE does not exist."
    exit 1
fi

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
FORGE_COMMAND="forge script --rpc-url http://127.0.0.1:$RPC_PORT scripts/L2_txn_simulation/$SCRIPT_FILE -vvvv --broadcast --private-key $PRIVATE_KEY --legacy"
echo "Running forge foundry script..."
eval $FORGE_COMMAND
echo "Forge script execution completed."
