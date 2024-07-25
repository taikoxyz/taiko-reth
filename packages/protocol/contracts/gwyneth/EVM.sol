// SPDX-License-Identifier: MIT

pragma solidity >=0.8.12 <0.9.0;

// EVM library
library EVM {
    // precompile addresses
    address constant xCallOptionsAddress = address(0x1100);

    uint constant l1ChainId = 1;
    uint constant version = 1;

    function xCallOnL1()
        public 
        view 
    {
        xCallOptions(l1ChainId);
    }

    function xCallOptions(uint chainID)
        public 
        view 
    {
        xCallOptions(chainID, true);
    }

    function xCallOptions(uint chainID, bool sandbox)
        public 
        view 
    {
        xCallOptions(chainID, sandbox, address(0), address(0));
    }

    function xCallOptions(uint chainID, bool sandbox, address txOrigin, address msgSender)
        public 
        view 
    {
        xCallOptions(chainID, sandbox, txOrigin, msgSender, 0x0, "");
    }

    function xCallOptions(uint chainID, bool sandbox, bytes32 blockHash, bytes memory proof)
        public 
        view 
    {
        xCallOptions(chainID, sandbox, address(0), address(0), blockHash, proof);
    }

    function xCallOptions(uint chainID, bool sandbox, address txOrigin, address msgSender, bytes32 blockHash, bytes memory proof)
        public 
        view 
    {
        // This precompile is not supported on L1
        require(chainID != l1ChainId);

        // Call the custom precompile
        bytes memory input = abi.encodePacked(version, chainID, sandbox, txOrigin, msgSender, blockHash, proof);
        (bool success, ) = xCallOptionsAddress.staticcall(input);
        require(success);
    }

    function isOnL1() public view returns (bool) {
        return chainId() == l1ChainId;
    }

    function chainId() public view returns (uint256) {
        return block.chainid;
    }
}