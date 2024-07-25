// SPDX-License-Identifier: MIT

pragma solidity >=0.8.12 <0.9.0;

import "./EVM.sol";

contract XChain {
    struct XChainCallProof {
        uint chainID;
        uint blockID;
        bytes callProof;
    }

    struct AsyncBusProof {
        uint busID;
        bytes boosterCallProof;
    }

    struct AsyncBusProofV2 {
        uint blockNumber;
        uint busID;
    }

    enum ProofType {
        INVALID,
        ASYNC,
        SYNC
    }

    // Messages are stored only on the source chain for ASYNC messages.
    // In SYNC mode, the message is stored on both the source and the target chain.
    bytes32[] public messages;

    // Only stored on L1
    // Currently getBlockHash() is not supported via the new Taiko Gwyneth
    //ITaiko public taiko;
    // todo (@Brecht): XChain has a bus property but Bus is an XChain (inherits). It does not make too much sense to me, or maybe i'm missing the point ?
    //Bus public bus;

    // Event that is logged when a transaction on a chain also needs to be executed on another chain
    event ExecuteNextOn(uint chainID, address from, address target, bytes callData);

    error FUNC_NOT_IMPLEMENTED();
    error NO_NEED_BUS_PROOF_ALL_ASYNC();

    function init(/*ITaiko _taiko*/)
        internal
    {
        //taiko = _taiko;
    }

    modifier notImplemented() {
        revert FUNC_NOT_IMPLEMENTED();
        _;
    }

    // xExecuteOn functions need
    // - to be external
    modifier xExecuteOn(uint chainID) {
        if (EVM.chainId() == chainID) {
            _;
        } else {
            EVM.xCallOptions(chainID, true);
            (bool success, bytes memory data) = address(this).staticcall(msg.data);
            require(success);
            // Just pass through the return data
            assembly {
                return(add(data, 32), mload(data))
            }
        }
    }

    // xFunctions functions need
    // - to be external
    // - to have `bytes proof` as the last function argument
    modifier xFunction(uint fromChainId, uint toChainId, bytes calldata proof) {
        // Current code is written with async case ! (This is outdated there, no need to run if running in sync. comp mode)
        if (fromChainId != toChainId) {
            // Remove the proof data from the message data
            // Bytes arays are padded to 32 bytes and start with a 32 byte length value
            uint messageLength = msg.data.length - ((proof.length + 31) / 32 + 1) * 32;
            bytes memory message = msg.data;
            assembly {
                mstore(message, messageLength)
            }

            // Use the bus to communicate between chains
            if (EVM.chainId() == fromChainId) {
                uint busID = write(message);

                // Always suggest doing an async proof for now on the target chain
                AsyncBusProofV2 memory asyncProof = AsyncBusProofV2({
                    busID: busID,
                    blockNumber: block.number
                });
                bytes memory encodedProof = abi.encode(asyncProof);
                bytes memory callData = bytes(string.concat(string(new bytes(0x0001)), string(message), string(encodedProof)));
                emit ExecuteNextOn(toChainId, address(0), address(this), callData);
            } else if (EVM.chainId() == toChainId) {
                consume(fromChainId, message, proof);
            } else {
                revert();
            }
        }
        _;
    }

    // These could also be exposed using a precompile because we could get them from public input, 
    // but that requires extra work so let's just fetch them from L1 for now
    function getBlockHash(uint chainID, uint blockID) external view xExecuteOn(EVM.l1ChainId) returns (bytes32) {
         // todo(@Brecht): Currently not supported or well, at least TaikoL1 does not have it with the current design.
        //return taiko.getBlockHash(chainID, blockID);
    }
    
    function calcMessageHash(bytes memory message) internal view returns (bytes32) {
        return keccak256(abi.encode(EVM.chainId(), msg.sender, message));
    }

    // Supports setting the call options using any L2 in the booster network.
    // This is done by first checking the validity of the blockhash of the specified L2.
    function xCallOptions(uint chainID, bool sandbox, bytes memory proof) internal view {
        // Decode the proof
        XChainCallProof memory chainCallProof = abi.decode(proof, (XChainCallProof));
        require(chainID == chainCallProof.chainID);

        // If the source chain isn't L1, go fetch the block header of the L2 stored on L1
        bytes32 blockHash = 0x0;
        if (chainID != EVM.l1ChainId) {
           
            blockHash = this.getBlockHash(chainID, chainCallProof.blockID);
        }

        // Do the call on the specified chain
        EVM.xCallOptions(chainID, sandbox, blockHash, chainCallProof.callProof);
    }

    // todo (@Brecht):
    // There was a circular reference (XBus inherits from XChain, while also XChain has a XBus property, so i made these to compile)
    // They will be inherited in XBus, but basically XBus can be incorporated into XChain, no ?

    // Question (Brecht):
    //- Shall we put back these functionalities to bus ?
    //- Shall we remove (as i did here) the ownership of the bus - then use the previous implementation ? (notImplemented modifier) and overwrite in the child "bus" ?

    // Currently, supposingly there is "synchronous composability", so let's assume a synchronous world
    function write(bytes memory message) public virtual notImplemented returns (uint) {}

    // Even tho the function just passes thru to write(), it is needed to bus-compatibility, where the consume function will differ
    function consume(uint256 /*fromChainId*/, bytes memory message, bytes calldata proof) public notImplemented virtual {}
}