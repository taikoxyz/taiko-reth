// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./TaikoL1TestBase.sol";

contract TaikoL1Test is TaikoL1TestBase {
    function deployTaikoL1() internal override returns (TaikoL1) {
        return
            TaikoL1(payable(deployProxy({ name: "taiko", impl: address(new TaikoL1()), data: "" })));
    }

    function test_L1_propose_prove_and_verify_blocks_sequentially() external {
        giveEthAndTko(Alice, 100 ether, 100 ether);

        TaikoData.BlockMetadata memory meta;

        vm.roll(block.number + 1);
        vm.warp(block.timestamp + 12);

        bytes32 parentMetaHash;
        bytes32 parentBlockHash = GENESIS_BLOCK_HASH;
        for (uint64 blockId = 1; blockId <= 20; blockId++) {
            printVariables("before propose & prove & verify");
            // Create metadata and propose the block
            meta = createBlockMetaData(Alice, blockId, 1, true, parentMetaHash, parentBlockHash);
            proposeBlock(Alice, meta, "");

            //Save arent data for next block iteration
            parentMetaHash = keccak256(abi.encode(meta));
            parentBlockHash = meta.blockHash;

            // Create proofs and prove a block
            bytes32 newRoot = randBytes32(); // Currently does not matter what do we feed as newRoot
                // as verification is mocked!!
            ChainProver.ProofBatch memory blockProofs =
                createProofs(uint64(block.number), newRoot, Alice, true);
            proveBlock(Alice, abi.encode(blockProofs));

            //Wait enought time and verify block
            vm.warp(uint32(block.timestamp + L1.SECURITY_DELAY_AFTER_PROVEN() + 1));
            vm.roll(block.number + 10);
            //verifyBlock(1);
            parentMetaHash = keccak256(abi.encode(meta));
            printVariables("after verify");
        }
    }

    function test_L1_propose_some_blocks_in_a_row_then_prove_and_verify() external {
        giveEthAndTko(Alice, 100 ether, 100 ether);

        TaikoData.BlockMetadata[] memory blockMetaDatas = new TaikoData.BlockMetadata[](20);

        vm.roll(block.number + 1);
        vm.warp(block.timestamp + 12);

        bytes32 parentMetaHash;
        bytes32 parentBlockHash = GENESIS_BLOCK_HASH;
        for (uint64 blockId = 1; blockId <= 20; blockId++) {
            printVariables("before propose & prove & verify");
            // Create metadata and propose the block
            blockMetaDatas[blockId - 1] =
                createBlockMetaData(Alice, blockId, 1, true, parentMetaHash, parentBlockHash);

            proposeBlock(Alice, blockMetaDatas[blockId - 1], "");

            //Save arent data for next block iteration
            parentMetaHash = keccak256(abi.encode(blockMetaDatas[blockId - 1]));
            parentBlockHash = blockMetaDatas[blockId - 1].blockHash;

            vm.roll(block.number + 1);
            vm.warp(block.timestamp + 12);
        }

        for (uint64 blockId = 1; blockId <= 20; blockId++) {
            bytes32 newRoot = randBytes32(); // Currently does not matter what do we feed as newRoot
                // as verification is mocked!!
            ChainProver.ProofBatch memory blockProofs =
                createProofs(uint64(block.number), newRoot, Alice, true);
            proveBlock(Alice, abi.encode(blockProofs));

            //Wait enought time and verify block (currently we simply just "wait enough" from latest
            // block and not time it perfectly)
            vm.warp(uint32(block.timestamp + L1.SECURITY_DELAY_AFTER_PROVEN() + 1));
            vm.roll(block.number + 10);
            //verifyBlock(1);
            parentMetaHash = keccak256(abi.encode(blockMetaDatas[blockId - 1]));
            printVariables("after verify 1");
        }
    }

    // This test does not fail anymore, because proposing is possible but validating of the preconf window violation will be done 
    // So for now, not needed ! Commenting out. When PR approved, i'll delete also.
    // function test_L1_propose_block_outside_the_4_epoch_window() external {
    //     giveEthAndTko(Alice, 100 ether, 100 ether);

    //     TaikoData.BlockMetadata memory meta;

    //     vm.roll(block.number + 1);
    //     vm.warp(block.timestamp + 12);

    //     bytes32 parentMetaHash;
    //     bytes32 parentBlockHash = GENESIS_BLOCK_HASH;
    //     // Create metadata and propose the block 129 blocks later only
    //     meta = createBlockMetaData(Alice, 1, 1, true, parentMetaHash, parentBlockHash);
    //     vm.roll(block.number + 129);
    //     vm.warp(block.timestamp + 129 * 12);

    //     proposeBlock(Alice, meta, TaikoErrors.L1_INVALID_L1_STATE_BLOCK.selector);
    // }

    function test_print_genesis_hash() external pure {
        console2.logBytes32(keccak256("GENESIS_BLOCK_HASH"));
    }
}
