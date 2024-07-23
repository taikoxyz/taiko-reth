// SPDX-License-Identifier: MIT

pragma solidity >=0.8.12 <0.9.0;

import "./XChain.sol";

contract XChainToken is XChain {
    // Only stored on L1
    uint private _totalBalance;
    // Stored on all chains
    mapping(address => uint) public balances;

    function totalBalance() 
        xExecuteOn(EVM.l1ChainId) 
        external
        view 
        returns (uint)  
    {
        return _totalBalance;
    }

    function xtransfer(address to, uint amount, uint256 fromChainId, uint256 toChainId, bytes calldata proof)
        xFunction(fromChainId, toChainId, proof)
        external
    {
        if (EVM.chainId() == fromChainId) {
            balances[msg.sender] -= amount;
        }
        if (EVM.chainId() == toChainId) {
            balances[to] += amount;
        }
    }
}