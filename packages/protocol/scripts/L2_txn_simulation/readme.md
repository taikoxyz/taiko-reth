# Create / simulate L2 transactions (propose transaction and an xtransfer of a dummy xChainToken)

In order to test the L2 node execution hook functionality, we need create valid L2 transactions and submit those to TaikoL1 - where a hook will be built in, to listen the proposeBlock and execute those transactions. This folder is to create L2 transactions (using the same pre-funded accounts Kurtosis is setting up by default) and submit it to our "L1" while using the local taiko_reth image as the EL.

## Prerequisites

Prerequisites can also be found in `deployments/local_deployment.md` file.

1. Testnet up and running:
```shell
kurtosis run github.com/ethpandaops/ethereum-package --args-file YOUR_PATH_TO_NETWORK_CONFIG/network_params.yaml
```

2. Main contracts deployed:
```shell
forge script --rpc-url http://127.0.0.1:PORT scripts/DeployL1Locally.s.sol -vvvv --broadcast --private-key PK --legacy
```
# ProposeBlock

## 1. Create and print L2 transactions ("off-chain")

Run script to gather 3 ether transactions, and print them out. `-n` flag stands for the nonce, and `-c` is for the L2 chainId.

```shell
$ python3 createL2Txns.py -n <CORRECT_L2_NONCE> -c <CORRECT_L2_CHAINID>
```

## 2. Prepare the script with proper data and fire away the L1 transaction

Edit the `ProposeBlock.s.sol` file to to set the valid `basedOperatorAddress` and also add the above generated 3 signed transactions (already in the `ProposeBlock.s.sol` file, not needed to run and add them, unless the network `id` or `nonce` is different), then fire away the L1 transaction with the script below:

```shell
$ forge script --rpc-url http://127.0.0.1:YOUR_PORT scripts/L2_txn_simulation/ProposeBlock.s.sol -vvvv --broadcast --private-key <YOUR_PRIVATE_KEY> --legacy
```

## 3. In case of TXN failure, you can get the error via the debug trace transaction RPC call

Command

```shell
curl http://127.0.0.1:YOUR_PORT \
-X POST \
-H "Content-Type: application/json" \
--data '{"method":"debug_traceTransaction","params":["YOUR_TXN_HASH", {"tracer": "callTracer"}], "id":1,"jsonrpc":"2.0"}'
```


# Send a dummy xChainToken

In order to send cross-chain transactions with `xCallOptions()`, when the network is up and running, deploy an `xChainERC20Token` contract and fire away an `xtransfer()` transaction.

```shell
forge script --rpc-url http://127.0.0.1:YOUR_PORT scripts/L2_txn_simulation/CreateXChainTxn.s.sol -vvvv --broadcast --private-key PK_IN_ENV_FILE --legacy
```