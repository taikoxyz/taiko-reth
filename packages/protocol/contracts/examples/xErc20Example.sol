// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "../gwyneth/XChainToken.sol";

contract xErc20Example is ERC20, XChainToken  {
    constructor() ERC20("xERC20", "xERC") {
        _mint(msg.sender, 100_000_000_000 * 10**18 );
    }
}