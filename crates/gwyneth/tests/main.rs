use ef_tests::{suite::Suite, Case};
use gwyneth::GwynethNode;
use reth_chainspec::{ChainSpec, ChainSpecBuilder};

pub mod cases;
pub mod test_payload;
use cases::SyncBlockchainTests;
use test_payload::*;

macro_rules! sync_db_access_test {
    ($test_name:ident, $dir:ident) => {
        #[test]
        fn $test_name() {
            SyncBlockchainTests::new(format!("GeneralStateTests/{}", stringify!($dir))).run();
        }
    };
}

macro_rules! l2_payload_test {
    ($test_name:ident, $dir:ident, $tx:expr) => {
        #[test]
        fn $test_name() {
            SyncBlockchainTests::new(format!("GeneralStateTests/{}", stringify!($dir))).run_l2($tx);
        }
    };
}

mod l2_payload_tests {
    use super::*;

    l2_payload_test!(shanghai, Shanghai, bullshit_l2_payload);
    l2_payload_test!(st_args_zero_one_balance, stArgsZeroOneBalance, bullshit_l2_payload);
}

mod sync_db_access_tests {
    use super::*;

    sync_db_access_test!(shanghai, Shanghai);
    sync_db_access_test!(st_args_zero_one_balance, stArgsZeroOneBalance);
    sync_db_access_test!(st_attack, stAttackTest);
    sync_db_access_test!(st_bad_opcode, stBadOpcode);
    sync_db_access_test!(st_bugs, stBugs);
    sync_db_access_test!(st_call_codes, stCallCodes);
    sync_db_access_test!(st_call_create_call_code, stCallCreateCallCodeTest);
    sync_db_access_test!(
        st_call_delegate_codes_call_code_homestead,
        stCallDelegateCodesCallCodeHomestead
    );
    sync_db_access_test!(st_call_delegate_codes_homestead, stCallDelegateCodesHomestead);
    sync_db_access_test!(st_chain_id, stChainId);
    sync_db_access_test!(st_code_copy_test, stCodeCopyTest);
    sync_db_access_test!(st_code_size_limit, stCodeSizeLimit);
    sync_db_access_test!(st_create2, stCreate2);
    sync_db_access_test!(st_create, stCreateTest);
    sync_db_access_test!(st_delegate_call_test_homestead, stDelegatecallTestHomestead);
    sync_db_access_test!(st_eip150_gas_prices, stEIP150singleCodeGasPrices);
    sync_db_access_test!(st_eip150, stEIP150Specific);
    sync_db_access_test!(st_eip158, stEIP158Specific);
    sync_db_access_test!(st_eip1559, stEIP1559);
    sync_db_access_test!(st_eip2930, stEIP2930);
    sync_db_access_test!(st_eip3607, stEIP3607);
    sync_db_access_test!(st_example, stExample);
    sync_db_access_test!(st_ext_codehash, stExtCodeHash);
    sync_db_access_test!(st_homestead, stHomesteadSpecific);
    sync_db_access_test!(st_init_code, stInitCodeTest);
    sync_db_access_test!(st_log, stLogTests);
    sync_db_access_test!(st_mem_expanding_eip150_calls, stMemExpandingEIP150Calls);
    sync_db_access_test!(st_memory_stress, stMemoryStressTest);
    sync_db_access_test!(st_memory, stMemoryTest);
    sync_db_access_test!(st_non_zero_calls, stNonZeroCallsTest);
    sync_db_access_test!(st_precompiles, stPreCompiledContracts);
    sync_db_access_test!(st_precompiles2, stPreCompiledContracts2);
    sync_db_access_test!(st_quadratic_complexity, stQuadraticComplexityTest);
    sync_db_access_test!(st_random, stRandom);
    sync_db_access_test!(st_random2, stRandom2);
    sync_db_access_test!(st_recursive_create, stRecursiveCreate);
    sync_db_access_test!(st_refund, stRefundTest);
    sync_db_access_test!(st_return, stReturnDataTest);
    sync_db_access_test!(st_revert, stRevertTest);
    sync_db_access_test!(st_self_balance, stSelfBalance);
    sync_db_access_test!(st_shift, stShift);
    sync_db_access_test!(st_sload, stSLoadTest);
    sync_db_access_test!(st_solidity, stSolidityTest);
    sync_db_access_test!(st_special, stSpecialTest);
    sync_db_access_test!(st_sstore, stSStoreTest);
    sync_db_access_test!(st_stack, stStackTests);
    sync_db_access_test!(st_static_call, stStaticCall);
    sync_db_access_test!(st_static_flag, stStaticFlagEnabled);
    sync_db_access_test!(st_system_operations, stSystemOperationsTest);
    sync_db_access_test!(st_time_consuming, stTimeConsuming);
    sync_db_access_test!(st_transaction, stTransactionTest);
    sync_db_access_test!(st_wallet, stWalletTest);
    sync_db_access_test!(st_zero_calls_revert, stZeroCallsRevert);
    sync_db_access_test!(st_zero_calls, stZeroCallsTest);
    sync_db_access_test!(st_zero_knowledge, stZeroKnowledge);
    sync_db_access_test!(st_zero_knowledge2, stZeroKnowledge2);
    sync_db_access_test!(vm_tests, VMTests);
}

// TODO: Add ValidBlocks and InvalidBlocks tests
