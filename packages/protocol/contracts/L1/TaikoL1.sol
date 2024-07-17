// SPDX-License-Identifier: MIT
//  _____     _ _         _         _
// |_   _|_ _(_) |_____  | |   __ _| |__ ___
//   | |/ _` | | / / _ \ | |__/ _` | '_ (_-<
//   |_|\__,_|_|_\_\___/ |____\__,_|_.__/__/

pragma solidity ^0.8.20;

import "../common/EssentialContract.sol";
import "./TaikoErrors.sol";
import "./preconfs/ISequencerRegistry.sol";
import "./TaikoEvents.sol";

/// @title TaikoL1
contract TaikoL1 is EssentialContract, TaikoEvents, TaikoErrors {
    event ProvingPaused(bool paused);

    uint256 public constant SECURITY_DELAY_AFTER_PROVEN = 8 hours;

    // According to EIP4844, each blob has up to 4096 field elements, and each
    // field element has 32 bytes.
    uint256 public constant MAX_BYTES_PER_BLOB = 4096 * 32;

    TaikoData.State public state;
    uint256[100] private __gap;

    /// @notice Initializes the rollup.
    /// @param _addressManager The {AddressManager} address.
    /// @param _genesisBlockHash The block hash of the genesis block.
    function init(
        address _owner,
        address _addressManager,
        bytes32 _genesisBlockHash
    )
        external
        initializer
    {
        __Essential_init(_owner, _addressManager);

        TaikoData.Config memory config = getConfig();
        require(isConfigValid(config), "invalid config");

        // Init state
        state.genesisHeight = uint64(block.number);
        state.genesisTimestamp = uint64(block.timestamp);
        state.numBlocks = 1;

        // Init the genesis block
        TaikoData.Block storage blk = state.blocks[0];
        blk.blockHash = _genesisBlockHash;
        blk.timestamp = uint64(block.timestamp);

        emit BlockVerified({ blockId: 0, blockHash: _genesisBlockHash });
    }

    /// @dev Proposes multiple Taiko L2 blocks.
    function proposeBlock(
        bytes[] calldata data,
        bytes[] calldata txLists
    )
        external
        payable
        nonReentrant
        whenNotPaused
        returns (TaikoData.BlockMetadata[] memory _blocks)
    {
        if (txLists.length != 0) {
            require(data.length == txLists.length, "mismatched params length");
        }

        _blocks = new TaikoData.BlockMetadata[](data.length);
        for (uint256 i = 0; i < data.length; i++) {
            if (txLists.length != 0) {
                // If calldata, then pass forward the calldata
                _blocks[i] =_proposeBlock(data[i], txLists[i]);
            } else {
                // Blob otherwise
                _blocks[i] = _proposeBlock(data[i], bytes(""));
            }

            // Check if we have whitelisted proposers
            if (!_isProposerPermitted(_blocks[i])) {
                revert L1_INVALID_PROPOSER();
            }
        }
    }

    /// Proposes a Taiko L2 block.
    /// @param data Block parameters, currently an encoded BlockMetadata object.
    /// @param txList txList data if calldata is used for DA.
    /// @return _block The metadata of the proposed L2 block.
    function _proposeBlock(
        bytes calldata data,
        bytes memory txList
    )
        private
        returns (
            TaikoData.BlockMetadata memory _block
        )
    {
        TaikoData.Config memory config = getConfig();

        // Decode the block data
        _block = abi.decode(data, (TaikoData.BlockMetadata));

        // Verify L1 data
        // TODO(Brecht): needs to be more configurable for preconfirmations
        require(_block.l1Hash == blockhash(_block.l1StateBlockNumber), "INVALID_L1_BLOCKHASH");
        require(_block.blockHash != 0x0, "INVALID_L2_BLOCKHASH");
        //require(_block.difficulty == block.prevrandao, "INVALID_DIFFICULTY");
        // Verify misc data
        require(_block.gasLimit == config.blockMaxGasLimit, "INVALID_GAS_LIMIT");

        require(_block.blobUsed == (txList.length == 0), "INVALID_BLOB_USED");
        // Verify DA data
        if (_block.blobUsed) {
            // Todo: Is blobHash posisble to be checked and pre-calculated in input metadata off-chain ?
            // or shall we do something with it to cross check ?
            // require(_block.blobHash == blobhash(0), "invalid data blob");
            require(
                uint256(_block.txListByteOffset) + _block.txListByteSize <= MAX_BYTES_PER_BLOB,
                "invalid blob size"
            );
        } else {
            require(_block.blobHash == keccak256(txList), "INVALID_TXLIST_HASH");
            require(_block.txListByteOffset == 0, "INVALID_TXLIST_START");
            require(_block.txListByteSize == uint24(txList.length), "INVALID_TXLIST_SIZE");
        }

        // Check that the tx length is non-zero and within the supported range
        require(
            _block.txListByteSize != 0 || _block.txListByteSize < config.blockMaxTxListBytes,
            "invalid txlist size"
        );

        TaikoData.Block storage parentBlock = state.blocks[(state.numBlocks - 1)];

        require(_block.parentMetaHash == parentBlock.metaHash, "invalid parentMetaHash");
        require(_block.parentBlockHash == parentBlock.blockHash, "invalid parentHash");

        // Verify the passed in L1 state block number.
        // We only allow the L1 block to be 4 epochs old.
        // The other constraint is that the L1 block number needs to be larger than or equal the one
        // in the previous L2 block.
        if (
            _block.l1StateBlockNumber + 128 < block.number
                || _block.l1StateBlockNumber >= block.number
                || _block.l1StateBlockNumber < parentBlock.l1StateBlockNumber
        ) {
            revert L1_INVALID_L1_STATE_BLOCK();
        }

        // Verify the passed in timestamp.
        // We only allow the timestamp to be 4 epochs old.
        // The other constraint is that the timestamp needs to be larger than or equal the one
        // in the previous L2 block.
        if (
            _block.timestamp + 128 * 12 < block.timestamp || _block.timestamp > block.timestamp
                || _block.timestamp < parentBlock.timestamp
        ) {
            revert L1_INVALID_TIMESTAMP();
        }

        // So basically we do not store these anymore!

        // // Create the block that will be stored onchain
        // TaikoData.Block memory blk = TaikoData.Block({
        //     blockHash: _block.blockHash,
        //     metaHash: keccak256(data),
        //     blockId: state.numBlocks,
        //     timestamp: _block.timestamp,
        //     l1StateBlockNumber: _block.l1StateBlockNumber
        // });

        // // Store the block
        // state.blocks[state.numBlocks] = blk;

        // // Store the passed in block hash as is
        // state.transitions[blk.blockId][_block.parentBlockHash].blockHash = _block.blockHash;
        // // Big enough number so that we are sure we don't hit that deadline in the future.
        // state.transitions[blk.blockId][_block.parentBlockHash].verifiableAfter = type(uint64).max;

        // // Increment the counter (cursor) by 1.
        // state.numBlocks++;

        emit BlockProposed({ blockId: _block.l2BlockNumber, meta: _block });
    }

    /// @notice Gets the details of a block.
    /// @param blockId Index of the block.
    /// @return blk The block.
    function getBlock(uint64 blockId) public view returns (TaikoData.Block memory) {
        //Todo (Brecht): we needed for some things like: BlockMetadata, used when parentBlock() was needed etc.
        return state.blocks[blockId];
    }

    function getLastVerifiedBlockId() public view returns (uint256) {
        return uint256(state.lastVerifiedBlockId);
    }

    function getNumOfBlocks() public view returns (uint256) {
        return uint256(state.numBlocks);
    }

    /// @notice Gets the configuration of the TaikoL1 contract.
    /// @return Config struct containing configuration parameters.
    function getConfig() public view virtual returns (TaikoData.Config memory) {
        return TaikoData.Config({
            chainId: 167_008,
            // Limited by the PSE zkEVM circuits.
            blockMaxGasLimit: 15_000_000,
            // Each go-ethereum transaction has a size limit of 128KB,
            // and right now txList is still saved in calldata, so we set it
            // to 120KB.
            blockMaxTxListBytes: 120_000
        });
    }

    function isConfigValid(TaikoData.Config memory config) public pure returns (bool) {
        if (
            config.chainId <= 1 //
                || config.blockMaxGasLimit == 0 || config.blockMaxTxListBytes == 0
                || config.blockMaxTxListBytes > 128 * 1024 // calldata up to 128K
        ) return false;

        return true;
    }

        // Additinal proposer rules
    function _isProposerPermitted(TaikoData.BlockMetadata memory _block) private returns (bool) {
        if (_block.l2BlockNumber == 1) {
            // Only proposer_one can propose the first block after genesis
            address proposerOne = resolve("proposer_one", true);
            if (proposerOne != address(0) && msg.sender != proposerOne) {
                return false;
            }
        }

        // If there's a sequencer registry, check if the block can be proposed by the current
        // proposer
        ISequencerRegistry sequencerRegistry =
            ISequencerRegistry(resolve("sequencer_registry", true));
        if (sequencerRegistry != ISequencerRegistry(address(0))) {
            if (!sequencerRegistry.isEligibleSigner(msg.sender)) {
                return false;
            }
        }
        return true;
    }
}
