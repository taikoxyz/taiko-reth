// SPDX-License-Identifier: MIT
//  _____     _ _         _         _
// |_   _|_ _(_) |_____  | |   __ _| |__ ___
//   | |/ _` | | / / _ \ | |__/ _` | '_ (_-<
//   |_|\__,_|_|_\_\___/ |____\__,_|_.__/__/

pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "forge-std/console2.sol";

import "../../contracts/examples/xERC20Example.sol";

contract CreateXChainTxn is Script {
    address public Bob_deployer_and_xchain_sender = 0x8943545177806ED17B9F23F0a21ee5948eCaa776; //Also .env PRIV_KEY is tied to Bob
    address public Alice_xchain_receiver = 0xE25583099BA105D9ec0A67f5Ae86D90e50036425;

    function run() external {
        vm.startBroadcast();
        
        //Deploy a contract and mints 100k for Bob
        xERC20Example exampleXChainToken = new xERC20Example("xChainExample", "xCE", Bob_deployer_and_xchain_sender, 100_000 * 1e18);

        // ChainId to send to
        uint256 dummyChainId = 12346; // Does not matter at this point

        console2.log("Sender balance (before sending):", exampleXChainToken.balanceOf(Bob_deployer_and_xchain_sender));
        exampleXChainToken.xtransfer(Alice_xchain_receiver, 2 * 1e18, block.chainid, dummyChainId);

        console2.log("Sender balance:", exampleXChainToken.balanceOf(Bob_deployer_and_xchain_sender));
        console2.log("Receiver balance:", exampleXChainToken.balanceOf(Alice_xchain_receiver));

        vm.stopBroadcast();
    }
}
