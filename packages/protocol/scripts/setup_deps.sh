#!/bin/bash

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if Docker daemon is running
is_docker_running() {
    docker info >/dev/null 2>&1
}

# Check for Docker installation and daemon status
if ! command_exists docker; then
    echo "Docker is not installed. Please install Docker first."
    exit 1
elif ! is_docker_running; then
    echo "Docker daemon is not running. Please start Docker first."
    exit 1
else
    echo "Docker is installed and running."
fi

# Check if the taiko_reth image exists
# if ! docker image inspect taiko_reth >/dev/null 2>&1; then
  echo "Docker image taiko_reth does not exist. Building the image..."
  if ! docker build ../../ -t taiko_reth; then
      echo "Failed to build the Docker image taiko_reth."
      exit 1
  fi
# else
#     echo "Docker image taiko_reth already exists."
# fi

# Function to install Kurtosis on macOS
install_kurtosis_mac() {
    if ! command_exists brew; then
        echo "Homebrew is not installed. Installing Homebrew..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    fi
    echo "Installing Kurtosis CLI with Homebrew..."
    brew install kurtosis-tech/tap/kurtosis-cli
}

# Function to install Kurtosis on Ubuntu
install_kurtosis_ubuntu() {
    echo "Installing Kurtosis CLI with apt..."
    echo "deb [trusted=yes] https://apt.fury.io/kurtosis-tech/ /" | sudo tee /etc/apt/sources.list.d/kurtosis.list
    sudo apt update
    sudo apt install -y kurtosis-cli
}

# Detect the operating system and install Kurtosis accordingly
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Detected macOS."
    install_kurtosis_mac
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        #if [[ "$ID" == "ubuntu" ]]; then
            echo "Detected Ubuntu."
            install_kurtosis_ubuntu
        #else
        #    echo "This script currently supports only Ubuntu and macOS."
        #    exit 1
        #fi
    else
        echo "This script currently supports only Ubuntu and macOS."
        exit 1
    fi
else
    echo "This script currently supports only Ubuntu and macOS."
    exit 1
fi

# Check if Kurtosis is installed and its version
if command_exists kurtosis; then
    KURTOSIS_VERSION=$(kurtosis version | grep -oP '(?<=CLI Version:\s)[\d.]+')
    echo "Kurtosis CLI is already installed. Version: $KURTOSIS_VERSION"
else
    echo "Kurtosis CLI installation failed or is not installed correctly."
    exit 1
fi

# Run the Kurtosis command and capture its output
echo "Running Kurtosis command..."
KURTOSIS_OUTPUT=$(kurtosis run github.com/adaki2004/ethereum-package --args-file ./scripts/confs/network_params.yaml)

# Extract the Blockscout port
BLOCKSCOUT_PORT=$(echo "$KURTOSIS_OUTPUT" | grep -A 5 "^[a-f0-9]\+ *blockscout " | grep "http:" | sed -E 's/.*-> http:\/\/127\.0\.0\.1:([0-9]+).*/\1/' | head -n 1)

if [ -z "$BLOCKSCOUT_PORT" ]; then
    echo "Failed to extract Blockscout port."
    exit 1
fi

echo "Extracted Blockscout port: $BLOCKSCOUT_PORT"
echo "$BLOCKSCOUT_PORT" > /tmp/kurtosis_blockscout_port
# # Print the entire Kurtosis output for debugging
echo "Kurtosis Output:"
echo "$KURTOSIS_OUTPUT"

# Extract the "User Services" section
USER_SERVICES_SECTION=$(echo "$KURTOSIS_OUTPUT" | awk '/^========================================== User Services ==========================================/{flag=1;next}/^$/{flag=0}flag')
# Print the "User Services" section for debugging
echo "User Services Section:"
echo "$USER_SERVICES_SECTION"
# Extract the dynamic port assigned to the rpc service for "el-1-reth-lighthouse"
RPC_PORT=$(echo "$USER_SERVICES_SECTION" | grep -A 5 "el-1-reth-lighthouse" | grep "rpc: 8545/tcp" | sed -E 's/.* -> 127.0.0.1:([0-9]+).*/\1/')
if [ -z "$RPC_PORT" ]; then
    echo "Failed to extract RPC port from User Services section."
    exit 1
else
    echo "Extracted RPC port: $RPC_PORT"
    echo "$RPC_PORT" > /tmp/kurtosis_rpc_port
fi

# Extract the Starlark output section
STARLARK_OUTPUT=$(echo "$KURTOSIS_OUTPUT" | awk '/^Starlark code successfully run. Output was:/{flag=1; next} /^$/{flag=0} flag')

# Extract the beacon_http_url for cl-1-lighthouse-reth
BEACON_HTTP_URL=$(echo "$STARLARK_OUTPUT" | jq -r '.all_participants[] | select(.cl_context.beacon_service_name == "cl-1-lighthouse-reth") | .cl_context.beacon_http_url')

if [ -z "$BEACON_HTTP_URL" ]; then
    echo "Failed to extract beacon_http_url for cl-1-lighthouse-reth."
    exit 1
else
    echo "Extracted beacon_http_url: $BEACON_HTTP_URL"
    echo "$BEACON_HTTP_URL" > /tmp/kurtosis_beacon_http_url
fi

# Find the correct Docker container
CONTAINER_ID=$(docker ps --format '{{.ID}} {{.Names}}' | grep 'el-1-reth-lighthouse--' | awk '{print $1}')

if [ -z "$CONTAINER_ID" ]; then
    echo "Failed to find the el-1-reth-lighthouse container."
    exit 1
else
    echo "Found container ID: $CONTAINER_ID"
fi

# Check if the file exists in the container
FILE_PATH="/app/rbuilder/config-gwyneth-reth.toml"
if ! docker exec "$CONTAINER_ID" test -f "$FILE_PATH"; then
    echo "File $FILE_PATH does not exist in the container."
    exit 1
fi

# Update the cl_node_url in the file, regardless of its current content
ESCAPED_URL=$(echo "$BEACON_HTTP_URL" | sed 's/[\/&]/\\&/g')
UPDATE_COMMAND="sed -i '/^cl_node_url[[:space:]]*=/c\cl_node_url = [\"$ESCAPED_URL\"]' $FILE_PATH"
if docker exec "$CONTAINER_ID" sh -c "$UPDATE_COMMAND"; then
    echo "Successfully updated $FILE_PATH in the container."
else
    echo "Failed to update $FILE_PATH in the container."
    exit 1
fi

# Verify the change
VERIFY_COMMAND="grep 'cl_node_url' $FILE_PATH"
VERIFICATION=$(docker exec "$CONTAINER_ID" sh -c "$VERIFY_COMMAND")
echo "Updated line in $FILE_PATH: $VERIFICATION"
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
FORGE_COMMAND="forge script --rpc-url http://127.0.0.1:$RPC_PORT scripts/DeployL1Locally.s.sol -vvvv --broadcast --private-key $PRIVATE_KEY --legacy"
echo "Running forge foundry script..."
FORGE_OUTPUT=$(eval $FORGE_COMMAND | tee /dev/tty)
echo "Script execution completed."


# Ensure the log file exists in the current working directory
touch ./rbuilder.log

echo "Starting rbuilder and streaming logs to ./rbuilder.log..."
docker exec -d "$CONTAINER_ID" /bin/bash -c "
    /app/start_rbuilder.sh > /tmp/rbuilder.log 2>&1 &
    RBUILDER_PID=\$!
    tail -f /tmp/rbuilder.log &
    TAIL_PID=\$!
    wait \$RBUILDER_PID
"

# Start a background process to stream logs from the container to the host file
docker exec "$CONTAINER_ID" tail -f /tmp/rbuilder.log >> ./rbuilder.log &
FILE_LOG_PID=$!

# Start another process to stream logs to the terminal
docker exec "$CONTAINER_ID" tail -f /tmp/rbuilder.log &
TERMINAL_LOG_PID=$!

# Set up a trap to handle Ctrl+C (SIGINT)
trap 'echo "Interrupt received. Stopping terminal log streaming, but file logging continues."; kill $TERMINAL_LOG_PID; exit' INT TERM

echo "rbuilder is running in the container."
echo "Logs are being streamed to ./rbuilder.log and to this terminal."
echo "Press Ctrl+C to stop watching logs in the terminal. rbuilder and file logging will continue."

# Wait for the terminal log streaming to be manually interrupted
wait $TERMINAL_LOG_PID

# Check if rbuilder is still running
if docker exec "$CONTAINER_ID" pgrep -f "/app/start_rbuilder.sh" > /dev/null; then
    echo "rbuilder is still running in the container. Logs continue to be written to ./rbuilder.log"
else
    echo "rbuilder has stopped unexpectedly."
    kill $FILE_LOG_PID
    exit 1
fi

# Extract the path to run-latest.json
RUN_LATEST_PATH=$(echo "$FORGE_OUTPUT" | grep "Transactions saved to:" | sed 's/Transactions saved to: //')

# Run the verification script
echo "Starting contract verification..."
BLOCKSCOUT_PORT=$(cat /tmp/kurtosis_blockscout_port)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
"$SCRIPT_DIR/verify_contracts.sh" "$BLOCKSCOUT_PORT" "$RUN_LATEST_PATH"
