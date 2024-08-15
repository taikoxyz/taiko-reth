// SPDX-License-Identifier: MIT
//  _____     _ _         _         _
// |_   _|_ _(_) |_____  | |   __ _| |__ ___
//   | |/ _` | | / / _ \ | |__/ _` | '_ (_-<
//   |_|\__,_|_|_\_\___/ |____\__,_|_.__/__/

pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "forge-std/console2.sol";

import "../../contracts/L1/TaikoL1.sol";

contract ProposeBlock is Script {
    address public taikoL1Address = 0x9fCF7D13d10dEdF17d0f24C62f0cf4ED462f65b7;//address(0);// TaikoL1 proxy address -> Get from the deployment
    address sender = 0x2c57d1CFC6d5f8E4182a56b4cf75421472eBAEa4; // With pre-generated eth

    function run() external {
        console2.log("Second batch");
        require(taikoL1Address != address(0), "based operator not set");

        vm.startBroadcast();
        
        bytes[] memory txLists = new bytes[](1);
        // The L2 chainId with which i encoded the TXNs were 167010
        // THe nonce was 0
        bytes memory firstAddressSendingNonce0 = hex"02f87683028c6280843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf241880de0b6b3a764000080c080a0d654805ce11de0ebc3882d0210f7d4df27cb65e8f1441f742814502c32fac90ca045902b6d5cd810b78ac4005d78b21ffa3fc02202f70af2b66acc2178357d5042";
        
        // The outcome of the above is the rlp encoded list (not concatenated but RLP encoded with: https://toolkit.abdk.consulting/ethereum#key-to-address,rlp)
        txLists[0] = hex"f87bb87902f87683028c6280843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf241880de0b6b3a764000080c080a0d654805ce11de0ebc3882d0210f7d4df27cb65e8f1441f742814502c32fac90ca045902b6d5cd810b78ac4005d78b21ffa3fc02202f70af2b66acc2178357d5042";

        bytes32 txListHash = keccak256(txLists[0]); //Since we not using Blobs, we need this

        // MetaData related
        bytes[] memory metasEncoded = new bytes[](1);
        TaikoData.BlockMetadata memory meta;
        console2.log(txLists[0].length);

        meta = createBlockMetaDataForFirstBlockDebug(sender, 2, uint64(block.timestamp), uint24(txLists[0].length), txListHash);

        metasEncoded[0] = abi.encode(meta);

        TaikoL1(taikoL1Address).proposeBlock{value: 0.1 ether }(metasEncoded, txLists);

        vm.stopBroadcast();
    }

    function createBlockMetaDataForFirstBlockDebug(
        address coinbase,
        uint64 l2BlockNumber,
        uint64 unixTimestamp,
        uint24 txListByteSize,
        bytes32 txListHash
    )
        internal
        returns (TaikoData.BlockMetadata memory meta)
    {
        meta.blockHash = 0xab80a9c4daa571aa308e967c9a6b4bf21ba8842d95d73d28be112b6fe0618e7c; // Randomly set it to smth

        //TaikoData.Block memory parentBlock = L1.getBlock(l2BlockNumber - 1);
        meta.parentMetaHash = 0x0000000000000000000000000000000000000000000000000000000000000000; // This is the genesis block's metaHash
        meta.parentBlockHash = 0xdf90a9c4daa571aa308e967c9a6b4bf21ba8842d95d73d28be112b6fe0618e8c; // This is the genesis block's blockhash
        meta.l1Hash = blockhash(40); //L1 private network's L1 blockheight, submit this block between 40 and 40+128 blcok of L1.
        meta.difficulty = block.prevrandao;
        meta.blobHash = txListHash;
        meta.coinbase = coinbase;
        meta.l2BlockNumber = l2BlockNumber;
        meta.gasLimit = 15_000_000;
        meta.l1StateBlockNumber = uint32(40); // Submit this block between 40 and 40+128 block of L1.
        meta.timestamp = unixTimestamp;

        meta.txListByteOffset = 0;
        meta.txListByteSize = txListByteSize; // Corresponding txn list byte size
        meta.blobUsed = false;
    }
}
