from web3 import Web3
from eth_abi import encode
import argparse

RPC_URL_L2 = 'http://127.0.0.1:8545' # Anything is fine for now as long as we dont have the L2 network, but if we have we can automate nonce and gas settings
w3_taiko_l2 = Web3(Web3.HTTPProvider(RPC_URL_L2)) 

# Some pre-loaded ETH addresses from Kurtosis private network (NO secret, no harm to use for private testnets!)
sender_addresses =  ['0x8943545177806ED17B9F23F0a21ee5948eCaa776', '0xE25583099BA105D9ec0A67f5Ae86D90e50036425', '0x614561D2d143621E126e87831AEF287678B442b8']
sender_pks = ['bcdf20249abf0ed6d944c0288fad489e33f66b3960d9e6229c1cd214ed3bbe31', '39725efee3fb28614de3bacaffe4cc4bd8c436257e2c8bb887c4b5c4be45e76d', '53321db7c1e331d93a11a41d16f004d7ff63972ec8ec7c25db329728ceeb1710']

receiver = '0xf93Ee4Cf8c6c40b329b0c0626F28333c132CF241' # This address also has pre-loaded ETH addresses

parser = argparse.ArgumentParser()

parser.add_argument("-n", "--nonce", help="collective nonce",
                    type=int, required=True)
parser.add_argument("-c", "--chainid", help="l2 chainId",
                    type=int, required=True)

transaction_list = []

if __name__ == "__main__":
    args = parser.parse_args()
    nonce = args.nonce
    chainId = args.chainid
    
    # Build the new tx list
    idx = 0
    for sender in sender_addresses:
        # Build the tx
        transaction = {
            'chainId': chainId,
            'from': sender,
            'to': receiver,
            'value': w3_taiko_l2.to_wei('1', 'ether'),
            'nonce': nonce, # later we can use something like: w3_taiko_l2.eth.get_transaction_count(address1),
            'gas': 200000,
            'maxFeePerGas': 2000000000, # w3_taiko_l2.eth.gas_price or something
            'maxPriorityFeePerGas': 1000000000,
        }

        # 2. Sign tx with a private key
        signed_txn = w3_taiko_l2.eth.account.sign_transaction(transaction, sender_pks[idx])
        
        # Most probably we need to zlib + rlp encode transactions not only just "concatenate"
        print("Txn ",idx, " bytes:")
        print(signed_txn.rawTransaction.hex())
        transaction_list.append(signed_txn)
        idx += 1