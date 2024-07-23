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
if ! docker image inspect taiko_reth >/dev/null 2>&1; then
    echo "Docker image taiko_reth does not exist. Building the image..."
    if ! docker build ../../ -t taiko_reth; then
        echo "Failed to build the Docker image taiko_reth."
        exit 1
    fi
else
    echo "Docker image taiko_reth already exists."
fi

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
        if [[ "$ID" == "ubuntu" ]]; then
            echo "Detected Ubuntu."
            install_kurtosis_ubuntu
        else
            echo "This script currently supports only Ubuntu and macOS."
            exit 1
        fi
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
KURTOSIS_OUTPUT=$(kurtosis run github.com/ethpandaops/ethereum-package --args-file ./scripts/confs/network_params.yaml)

# Print the entire Kurtosis output for debugging
# echo "Kurtosis Output:"
# echo "$KURTOSIS_OUTPUT"

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
eval $FORGE_COMMAND

echo "Script execution completed."
