// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../gwyneth/XChainERC20Token.sol";

contract xERC20Example is XChainERC20Token  {
    constructor(string memory name_, string memory symbol_, address premintAddress_, uint256 premintAmount_ ) XChainERC20Token(name_, symbol_, premintAddress_, premintAmount_ ) {}
}