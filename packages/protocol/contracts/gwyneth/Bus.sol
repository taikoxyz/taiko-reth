// SPDX-License-Identifier: MIT

pragma solidity >=0.8.12 <0.9.0;

import "./XChain.sol";

contract Bus is XChain {
    // Stored only on the target chain
    mapping (bytes32 => bool) public consumed;

    function isMessageSent(bytes32 messageHash, uint busID) external view returns (bool) {
        return messages[busID] == messageHash;
    }

    function write(bytes memory message) public override returns (uint) {
        messages.push(calcMessageHash(message));
        return messages.length - 1;
    }

    function consume(uint fromChainId, bytes memory message, bytes calldata proof) public override {
        ProofType proofType = ProofType(uint16(bytes2(proof[:2])));
        if (proofType == ProofType.ASYNC) {
            // Decode the proof
            AsyncBusProof memory busProof = abi.decode(proof[2:], (AsyncBusProof));

            // Calculate the message hash
            bytes32 messageHash = calcMessageHash(message);

            // Do the call on the source chain to see if the message was sent there
            xCallOptions(fromChainId, true, busProof.boosterCallProof);
            bool isSent = this.isMessageSent(messageHash, busProof.busID);
            require(isSent == true);

            // Make sure this is the first and last time this message is consumed
            require(consumed[messageHash] == false);
            consumed[messageHash] = true;
        } else if (proofType == ProofType.SYNC) {
            // Sync system with shared validity (e.g.: like a SignalService shared validity thing)
            write(message);
        } else {
            revert("INVALID BUS PROOF");
        }
    }
}