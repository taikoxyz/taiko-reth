// SPDX-License-Identifier: MIT
//  _____     _ _         _         _
// |_   _|_ _(_) |_____  | |   __ _| |__ ___
//   | |/ _` | | / / _ \ | |__/ _` | '_ (_-<
//   |_|\__,_|_|_\_\___/ |____\__,_|_.__/__/

pragma solidity ^0.8.20;

import "../common/EssentialContract.sol";
import "../libs/LibAddress.sol";
import "./TaikoData.sol";
import "./TaikoErrors.sol";
import "./VerifierRegistry.sol";
import "./verifiers/IVerifier.sol";

/// @title ChainProver
/// @notice The prover contract for Taiko.
contract ChainProver is EssentialContract, TaikoErrors {
    using LibAddress for address;


    /// @dev Struct representing transition to be proven.
    struct ProofData {
        IVerifier verifier;
        bytes proof;
    }

    /// @dev Struct representing transition to be proven.
    struct ProofBatch {
        TaikoData.BlockMetadata blockMetadata; //Maybe needed (?)
        bytes32 newStateHash; // keccak(new_l1_blockhash, new_root))
        ProofData[] proofs;
        address prover;
    }

    // New, and only state var
    bytes32 public currentStateHash; //keccak(l1_blockhash, root)

    function init(address _owner, address _addressManager) external initializer {
        if (_addressManager == address(0)) {
            revert L1_INVALID_ADDRESS();
        }
        __Essential_init(_owner, _addressManager);
    }

    /// @dev Proposes a Taiko L2 block.
    function proveBlock(bytes calldata data) external nonReentrant whenNotPaused {
        // Decode the block data
        ProofBatch memory proofBatch = abi.decode(data, (ProofBatch));

        VerifierRegistry verifierRegistry = VerifierRegistry(resolve("verifier_registry", false));
        // Verify the proofs
        uint160 prevVerifier = uint160(0);
        for (uint256 i = 0; i < proofBatch.proofs.length; i++) {
            IVerifier verifier = proofBatch.proofs[i].verifier;
            // Make sure each verifier is unique
            if (prevVerifier >= uint160(address(verifier))) {
                revert L1_INVALID_OR_DUPLICATE_VERIFIER();
            }
            // Make sure it's a valid verifier
            require(verifierRegistry.isVerifier(address(verifier)), "invalid verifier");
            // Verify the proof
            verifier.verifyProof(
                keccak256(abi.encode(proofBatch.blockMetadata)), //Maybe block metadata (?) also an input ?
                keccak256(abi.encode(currentStateHash, proofBatch.newStateHash)),
                proofBatch.prover,
                proofBatch.proofs[i].proof
            );
            prevVerifier = uint160(address(verifier));
        }

        // Make sure the supplied proofs are sufficient.
        // Can use some custom logic here. but let's keep it simple
        require(proofBatch.proofs.length >= 3, "insufficient number of proofs");

        currentStateHash = proofBatch.newStateHash;

       //todo(@Brecht) How do we detect (or poison) verifiers or allow the chain to be corrected if the proof verifiers are buggy ?


    }
}
