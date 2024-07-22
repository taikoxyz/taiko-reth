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
    address sender = 0x8943545177806ED17B9F23F0a21ee5948eCaa776; // With pre-generated eth

    function run() external {

        require(taikoL1Address != address(0), "based operator not set");

        vm.startBroadcast();
        
        bytes[] memory txLists = new bytes[](1);
        // The L2 chainId with which i encoded the TXNs were 167011
        // THe nonce was 0
        bytes memory firstAddressSendingNonce0 = hex"02f87683028c6380843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf24188016345785d8a000080c080a08f0f52d943504cecea0d6ce317c2fde8b0c27b1e449d85fcf98ccd2f50ac804ba04d5d56356518c1de0c1ece644a8a2fe64e6cc136cd8db0a21a21f72c167353c6";
        bytes memory secondAddressSendingNonce0 = hex"02f87683028c6380843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf24188016345785d8a000080c080a0622e7060e09afd2100784bdc88ebb838729128bb6eb40f8b7f458430d56dafd4a006fe5d1a466788f941020a2278860c3f2642e44108c666ecd25b30d1b2f7a420";
        bytes memory thirdAddressSendingNonce0 = hex"02f87683028c6380843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf24188016345785d8a000080c001a0558488f3af91777c382d2ab6ac3507f5d6b906431534193c1a45cc2a08b2825ea0495efd571c9ea5a5290f10efaa219f8c31b4e714745737c4e019df76f7a6df4b";

        // The outcome of the above is the rlp encoded list (not concatenated but RLP encoded with: https://toolkit.abdk.consulting/ethereum#key-to-address,rlp)
        txLists[0] = hex"f90171b87902f87683028c6280843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf241880de0b6b3a764000080c080a07f983645ddf8365d14e5fb4e3b07c19fe31e23edd9ee4a737388acc2da7e64a3a072a56043512806a6de5f66f28bb659236eea41c9d66db8493f436804c42723d3b87902f87683028c6280843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf241880de0b6b3a764000080c001a030911ab2ebf76f1e1bfe00d721207d929053efb051d50708a10dd9f66f84bacba07705a7cdb86ff00aa8c131ef3c4cb2ea2f2f4730d93308f1afbb94a04c1c9ae9b87902f87683028c6280843b9aca00847735940083030d4094f93ee4cf8c6c40b329b0c0626f28333c132cf241880de0b6b3a764000080c001a07da8dfb5bc3b7b353f9614bcd83733168500d1e06f2bcdac761cc54c85847e6aa03b041b0605e86aa379ff0f58a60743da411dfd1a9d4f1d18422a862f67a57fee";

        bytes32 txListHash = keccak256(txLists[0]); //Since we not using Blobs, we need this

        // MetaData related
        bytes[] memory metasEncoded = new bytes[](1);
        TaikoData.BlockMetadata memory meta;
        console2.log(txLists[0].length);

        meta = createBlockMetaDataForFirstBlockDebug(sender, 1, uint64(block.timestamp), uint24(txLists[0].length), txListHash);

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
        meta.l1Hash = blockhash(30); //L1 private network's L1 blockheight, submit this block between 30 and 30+128 blcok of L1.
        meta.difficulty = block.prevrandao;
        meta.blobHash = txListHash;
        meta.coinbase = coinbase;
        meta.l2BlockNumber = l2BlockNumber;
        meta.gasLimit = 15_000_000;
        meta.l1StateBlockNumber = uint32(30); // Submit this block between 30 and 30+128 blcok of L1.
        meta.timestamp = unixTimestamp;

        meta.txListByteOffset = 0;
        meta.txListByteSize = txListByteSize; // Corresponding txn list byte size
        meta.blobUsed = false;
    }
}
