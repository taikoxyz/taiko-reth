// SPDX-License-Identifier: MIT

pragma solidity >=0.8.12 <0.9.0;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "./XChain.sol";

// The reason we need this is because i realized we need to somehow 'override' some of the functions we have in ERC20, and since the balances need to be affected in ERC20 and XChainToken, it is not possible with the current standard, except if we linearize the inheritance (ERC20 -> XChainToken -> TokenImplementation)
contract XChainERC20Token is XChain, ERC20 {
    // Only stored on L1
    // @Brecht -> Shall we overwrite in our xERC20Example the totalSupply() of ERC20 ? And use this var instead of the ERC20's _totalSupply
    // Not sure in this because i guess it shall serve the same purpose as totalSupply(), also it is a completely different interaction (on xChain) than on the canonical chain, but the totalSupply shall be the same IMO.
    //meeting meinutes: We can get rid of this.
    uint private _totalBalance;
    // Stored on all chains
    // This lead me to realize we need thi sinheritance:
    // Somehow this has to overwrite (or rather be used) in the ERC20 contract, right ? Like with the balanceOf(addr), otherwise the erc20 is not 'notified'.
    // What if we have a function in child ERC20.. which needs to be implemented, like modifyERC20Balance();
    // Example:
    // BOb does an xTransfer to Alice from cahin A to chain B. It is is OK but it shall translate into an ERC20 balance change too, not only in this contract but in the ERC20 contract which is with the prev. inheritance was not possible. 
    /*New variables - overriden from ERC20 since we want them to be modifiable*/
    mapping(address => uint) private _balances; // -> Need to redefine and override functions
    uint256 private _totalSupply; // -> Need to redefine and override functions

    constructor(string memory name_, string memory symbol_, address premintAddress_, uint256 premintAmount_ ) ERC20(name_, symbol_) {
        _mint(premintAddress_, premintAmount_);
    }

    // xtransfer for async case (proof needed)
    function xtransfer(address to, uint amount, uint256 fromChainId, uint256 toChainId, bytes calldata proof)
        xFunction(fromChainId, toChainId, proof)
        external
    {
        if (EVM.chainId() == fromChainId) {
            _balances[msg.sender] -= amount;
        }
        if (EVM.chainId() == toChainId) {
            _balances[to] += amount;
        }
    }

    // xtransfer for async case
    function xtransfer(address to, uint amount, uint256 fromChainId, uint256 toChainId)
        external
    {
        require(EVM.chainId() == fromChainId, "ASYNC_CASE, only call it on source chain");

        _balances[msg.sender] -= amount;
        // We need to do xCallOptions (incoprpotate the minting on the dest chain)
        // We chack we are on the corect sourvce chain and then we do evm.
        EVM.xCallOptions(toChainId);
        this.xmint(to, amount);
    }

    // DO a mind-puzzle with Brecht if this is really solving the problems of Alice sending Bob from chainA to chainB some tokens!!
    // Mint function -> Should only be called by the SC itself.
    function xmint(address to, uint amount)
        external
    {
        // Only be called by itself (internal bookikeeping)
        require(msg.sender == address(this), "NOT_ALLOWED");
        _balances[to] += amount;
    }

    /* Overrides of ERC20 */
    //Change totalSupply and apply xExecuteOn modifier
    function totalSupply() //Is it the same as totalSupply() if so, i think that shall be fine!
        xExecuteOn(EVM.l1ChainId) //why it has an xExecuteOn modifier ? And why it is applied only here ?
        public
        view
        override
        returns (uint256)
    {
        return _totalSupply;
    }

    function balanceOf(address account) public view virtual override returns (uint256) {
        return _balances[account];
    }

    /**
     * @dev Moves `amount` of tokens from `from` to `to`.
     *
     * This internal function is equivalent to {transfer}, and can be used to
     * e.g. implement automatic token fees, slashing mechanisms, etc.
     *
     * Emits a {Transfer} event.
     *
     * Requirements:
     *
     * - `from` cannot be the zero address.
     * - `to` cannot be the zero address.
     * - `from` must have a balance of at least `amount`.
     */
    function _transfer(address from, address to, uint256 amount) internal virtual override {
        require(from != address(0), "ERC20: transfer from the zero address");
        require(to != address(0), "ERC20: transfer to the zero address");

        _beforeTokenTransfer(from, to, amount);

        uint256 fromBalance = _balances[from];
        require(fromBalance >= amount, "ERC20: transfer amount exceeds balance");
        unchecked {
            _balances[from] = fromBalance - amount;
            // Overflow not possible: the sum of all balances is capped by totalSupply, and the sum is preserved by
            // decrementing then incrementing.
            _balances[to] += amount;
        }

        emit Transfer(from, to, amount);

        _afterTokenTransfer(from, to, amount);
    }

    /** @dev Creates `amount` tokens and assigns them to `account`, increasing
     * the total supply.
     *
     * Emits a {Transfer} event with `from` set to the zero address.
     *
     * Requirements:
     *
     * - `account` cannot be the zero address.
     */
    function _mint(address account, uint256 amount) internal virtual override {
        require(account != address(0), "ERC20: mint to the zero address");

        _beforeTokenTransfer(address(0), account, amount);

        _totalSupply += amount;
        unchecked {
            // Overflow not possible: balance + amount is at most totalSupply + amount, which is checked above.
            _balances[account] += amount;
        }
        emit Transfer(address(0), account, amount);

        _afterTokenTransfer(address(0), account, amount);
    }

    /**
     * @dev Destroys `amount` tokens from `account`, reducing the
     * total supply.
     *
     * Emits a {Transfer} event with `to` set to the zero address.
     *
     * Requirements:
     *
     * - `account` cannot be the zero address.
     * - `account` must have at least `amount` tokens.
     */
    function _burn(address account, uint256 amount) internal virtual override {
        require(account != address(0), "ERC20: burn from the zero address");

        _beforeTokenTransfer(account, address(0), amount);

        uint256 accountBalance = _balances[account];
        require(accountBalance >= amount, "ERC20: burn amount exceeds balance");
        unchecked {
            _balances[account] = accountBalance - amount;
            // Overflow not possible: amount <= accountBalance <= totalSupply.
            _totalSupply -= amount;
        }

        emit Transfer(account, address(0), amount);

        _afterTokenTransfer(account, address(0), amount);
    }
}