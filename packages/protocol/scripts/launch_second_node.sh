#!/bin/bash

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check for Docker installation
if ! command_exists docker; then
    echo "Docker is not installed. Please install Docker first."
    exit 1
fi

# Function to get container ID by name prefix
get_container_id() {
    docker ps --format '{{.ID}}' --filter "name=$1"
}

# Function to copy file from container to host
copy_from_container() {
    docker cp "$1:$2" "$3"
}

# Function to get network name from container
get_network_name() {
    docker inspect -f '{{range $key, $value := .NetworkSettings.Networks}}{{$key}}{{end}}' "$1"
}

# Function to get container IP address
get_container_ip() {
    docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$1"
}

clean_directory() {
    if [ -d "$1" ]; then
        echo "Cleaning directory: $1"
        rm -rf "$1"/*
    else
        echo "Creating directory: $1"
        mkdir -p "$1"
    fi
}

# Function to get or create JWT secret
get_or_create_jwt_secret() {
    local jwt_path="$HOME/jwt/jwtsecret"
    if [ -f "$jwt_path" ]; then
        echo "Using existing JWT secret."
    else
        echo "Creating new JWT secret."
        mkdir -p "$(dirname "$jwt_path")"
        openssl rand -hex 32 | tr -d "\n" > "$jwt_path"
    fi
    echo "$jwt_path"
}

# Get or create JWT secret
JWT_SECRET_PATH=$(get_or_create_jwt_secret)
echo "JWT secret path: $JWT_SECRET_PATH"

# Get container IDs
EL_CONTAINER_ID=$(get_container_id "el-2-reth-teku")
CL_CONTAINER_ID=$(get_container_id "cl-2-teku-reth")

if [ -z "$EL_CONTAINER_ID" ] || [ -z "$CL_CONTAINER_ID" ]; then
    echo "Failed to find required containers."
    exit 1
fi

# Get network name
NETWORK_NAME=$(get_network_name "$EL_CONTAINER_ID")
if [ -z "$NETWORK_NAME" ]; then
    echo "Failed to get network name."
    exit 1
fi
echo "Using network: $NETWORK_NAME"

# Get EL container IP
EL_IP=$(get_container_ip "$EL_CONTAINER_ID")

# Get bootnode from EL container
BOOTNODE=$(docker exec "$EL_CONTAINER_ID" ps aux | grep docker-init | grep -o 'bootnodes=[^ ]*' | cut -d= -f2)

# Get CL bootnode
CL_BOOTNODE=$(docker exec "$CL_CONTAINER_ID" ps aux | grep docker-init | grep -o 'p2p-discovery-bootnodes=[^ ]*' | cut -d= -f2)

# Clean and recreate required directories
clean_directory ~/data/reth/execution-data
clean_directory ~/data/teku/teku-beacon-data
clean_directory ~/data/teku/validator-keys/teku-secrets
clean_directory ~/data/teku/validator-keys/teku-keys

# Create required directories
mkdir -p ~/network-configs ~/jwt

# Copy required files
copy_from_container "$EL_CONTAINER_ID" "/network-configs/genesis.json" ~/network-configs/
copy_from_container "$CL_CONTAINER_ID" "/network-configs/genesis.ssz" ~/network-configs/
copy_from_container "$CL_CONTAINER_ID" "/network-configs/config.yaml" ~/network-configs/

# Launch EL container
echo "Launching EL container..."
EL_CONTAINER_ID=$(docker run -d --name reth-node3 --network "$NETWORK_NAME" \
    -v ~/data/reth/execution-data:/data/reth/execution-data \
    -v ~/network-configs:/network-configs \
    -v ~/jwt:/jwt \
    -p 8545:8545 \
    -p 10110:10110 \
    taiko_reth node -vvv --datadir=/data/reth/execution-data \
    --chain=/network-configs/genesis.json \
    --http --http.port=8545 --http.addr=0.0.0.0 \
    --http.corsdomain="*" --http.api=admin,net,eth,web3,debug,trace \
    --ws --ws.addr=0.0.0.0 --ws.port=8550 --ws.api=net,eth \
    --ws.origins="*" --nat=extip:0.0.0.0 \
    --authrpc.port=8551 --authrpc.jwtsecret=/jwt/jwtsecret \
    --authrpc.addr=0.0.0.0 --metrics=0.0.0.0:9003 \
    --discovery.port=42011 --port=42011 \
    --bootnodes="$BOOTNODE")

if [ -z "$EL_CONTAINER_ID" ]; then
    echo "Failed to launch EL container."
    exit 1
fi

# Get the IP of the newly launched EL container
NEW_EL_IP=$(get_container_ip "$EL_CONTAINER_ID")
if [ -z "$NEW_EL_IP" ]; then
    echo "Failed to get IP of the new EL container."
    exit 1
fi

echo "New EL container IP: $NEW_EL_IP"

# Wait for the EL container to be ready (you might want to implement a more robust check)
sleep 10

# Launch CL container
echo "Launching CL container..."
docker run -d \
  --name teku-node2 \
  --network "$NETWORK_NAME" \
  -v ~/data/teku/teku-beacon-data:/data/teku/teku-beacon-data \
  -v ~/data/teku/validator-keys:/validator-keys/ \
  -v ~/network-configs:/network-configs \
  -v ~/jwt:/jwt/ \
  --entrypoint /bin/sh \
  consensys/teku:latest -c "
    MY_IP=\$(hostname -i) && \
    exec /opt/teku/bin/teku \
    --logging=INFO \
    --log-destination=CONSOLE \
    --network=/network-configs/config.yaml \
    --data-path=/data/teku/teku-beacon-data \
    --data-storage-mode=ARCHIVE \
    --p2p-enabled=true \
    --p2p-peer-lower-bound=1 \
    --p2p-advertised-ip=\$MY_IP \
    --p2p-discovery-site-local-addresses-enabled=true \
    --p2p-port=9000 \
    --rest-api-enabled=true \
    --rest-api-docs-enabled=true \
    --rest-api-interface=0.0.0.0 \
    --rest-api-port=4000 \
    --rest-api-host-allowlist=* \
    --data-storage-non-canonical-blocks-enabled=true \
    --ee-jwt-secret-file=/jwt/jwtsecret \
    --ee-endpoint=http://$NEW_EL_IP:8551 \
    --metrics-enabled \
    --metrics-interface=0.0.0.0 \
    --metrics-host-allowlist='*' \
    --metrics-categories=BEACON,PROCESS,LIBP2P,JVM,NETWORK,PROCESS \
    --metrics-port=8008 \
    --ignore-weak-subjectivity-period-enabled=true \
    --initial-state=/network-configs/genesis.ssz \
    --p2p-discovery-bootnodes=$CL_BOOTNODE \
    --validator-keys=/validator-keys/teku-keys:/validator-keys/teku-secrets \
    --validators-proposer-default-fee-recipient=0x8943545177806ED17B9F23F0a21ee5948eCaa776 \
    --validators-graffiti=2-reth-teku
  "

echo "Second node (EL and CL) launched successfully!"