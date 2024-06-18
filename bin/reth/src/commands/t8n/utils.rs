use eyre::{eyre, ContextCompat, Report};
#[cfg(feature = "optimism")]
use reth_primitives::TxDeposit;
use reth_primitives::{
    sign_message, Address, Signature as PrimitiveSignature, Transaction as PrimitiveTransaction,
    TransactionKind, TransactionSigned, TxEip1559, TxEip2930, TxEip4844, TxLegacy, TxType, B256,
    U256,
};
#[cfg(feature = "optimism")]
use reth_rpc_types::optimism::OptimismTransactionFields;
use reth_rpc_types::{AccessList, Signature, Transaction};
use secp256k1::SecretKey;

// Get the `odd_y_parity` from the `v` value depends on chain_id
#[inline]
fn get_odd_y_parity(v: u64, chain_id: Option<u64>) -> bool {
    if let Some(chain_id) = chain_id {
        // EIP-155: v = {0, 1} + CHAIN_ID * 2 + 35
        v - chain_id * 2 - 35 == 1
    } else {
        v - 27 == 1
    }
}

fn to_legacy_primitive_signature(
    signature: Signature,
    chain_id: Option<u64>,
) -> PrimitiveSignature {
    PrimitiveSignature {
        r: signature.r,
        s: signature.s,
        odd_y_parity: get_odd_y_parity(signature.v.to(), chain_id),
    }
}

fn to_primitive_transaction_kind(to: Option<Address>) -> TransactionKind {
    match to {
        Some(to) => TransactionKind::Call(to),
        None => TransactionKind::Create,
    }
}

fn to_typed_primitive_signature(signature: Signature) -> PrimitiveSignature {
    PrimitiveSignature {
        r: signature.r,
        s: signature.s,
        odd_y_parity: signature.v == U256::from(1),
    }
}

fn to_primitive_signature(
    signature: Signature,
    tx_type: TxType,
    chain_id: Option<u64>,
) -> PrimitiveSignature {
    match tx_type {
        TxType::Legacy => to_legacy_primitive_signature(signature, chain_id),
        _ => to_typed_primitive_signature(signature),
    }
}
#[cfg(feature = "optimism")]
fn try_into_optimism_fields(other: OtherFields) -> eyre::Result<OptimismTransactionFields> {
    let value = serde_json::to_value(other)?;
    Ok(serde_json::from_value(value)?)
}

/// Convert [Transaction] to [TransactionSigned]
pub(crate) fn try_into_primitive_transaction_and_sign(
    tx: Transaction,
    secret: &Option<SecretKey>,
) -> eyre::Result<TransactionSigned> {
    let tx_type: TxType = tx
        .transaction_type
        .map(|v| v.to::<u8>())
        .unwrap_or_default()
        .try_into()
        .map_err(Report::msg)?;
    let chain_id = tx.chain_id.map(|v| v.to());
    let transaction = match tx_type {
        TxType::Legacy => PrimitiveTransaction::Legacy(TxLegacy {
            chain_id,
            nonce: tx.nonce.to(),
            gas_price: tx.gas_price.map(|v| v.to()).context("missing gas_price")?,
            gas_limit: tx.gas.to(),
            to: to_primitive_transaction_kind(tx.to),
            value: tx.value,
            input: tx.input,
        }),
        TxType::Eip2930 => PrimitiveTransaction::Eip2930(TxEip2930 {
            chain_id: chain_id.context("missing chain_id")?,
            nonce: tx.nonce.to(),
            gas_price: tx.gas_price.map(|v| v.to()).context("missing gas_price")?,
            gas_limit: tx.gas.to(),
            to: to_primitive_transaction_kind(tx.to),
            value: tx.value,
            access_list: tx
                .access_list
                .map(|v| AccessList(v).into())
                .context("missing access_list")?,
            input: tx.input,
        }),
        TxType::Eip1559 => PrimitiveTransaction::Eip1559(TxEip1559 {
            chain_id: chain_id.context("missing chain_id")?,
            nonce: tx.nonce.to(),
            gas_limit: tx.gas.to(),
            max_fee_per_gas: tx
                .max_fee_per_gas
                .map(|v| v.to())
                .context("missing max_fee_per_gas")?,
            max_priority_fee_per_gas: tx
                .max_priority_fee_per_gas
                .map(|v| v.to())
                .context("missing max_priority_fee_per_gas")?,
            to: to_primitive_transaction_kind(tx.to),
            value: tx.value,
            access_list: tx
                .access_list
                .map(|v| AccessList(v).into())
                .context("missing access_list")?,
            input: tx.input,
            is_anchor: false,
        }),
        TxType::Eip4844 => PrimitiveTransaction::Eip4844(TxEip4844 {
            chain_id: chain_id.context("missing chain_id")?,
            nonce: tx.nonce.to(),
            gas_limit: tx.gas.to(),
            max_fee_per_gas: tx
                .max_fee_per_gas
                .map(|v| v.to())
                .context("missing max_fee_per_gas")?,
            max_priority_fee_per_gas: tx
                .max_priority_fee_per_gas
                .map(|v| v.to())
                .context("missing max_priority_fee_per_gas")?,
            to: to_primitive_transaction_kind(tx.to),
            value: tx.value,
            access_list: tx
                .access_list
                .map(|v| AccessList(v).into())
                .context("missing access_list")?,
            input: tx.input,
            blob_versioned_hashes: tx.blob_versioned_hashes,
            max_fee_per_blob_gas: tx
                .max_fee_per_blob_gas
                .map(|v| v.to())
                .context("missing max_fee_per_blob_gas")?,
        }),
        #[cfg(feature = "optimism")]
        TxType::Deposit => {
            let other = try_into_optimism_fields(tx.other)?;
            PrimitiveTransaction::Deposit(TxDeposit {
                source_hash: other.source_hash.context("missing source_hash")?,
                from: tx.from,
                to: to_primitive_transaction_kind(tx.to),
                mint: other.mint.map(|v| v.to()),
                value: tx.value,
                gas_limit: tx.gas.to(),
                is_system_transaction: other.is_system_tx.context("missing is_system_tx")?,
                input: tx.input,
            })
        }
    };
    let signature = match (tx.signature, secret) {
        (Some(signature), _) => to_primitive_signature(signature, tx_type, chain_id),
        (None, Some(secret)) => {
            let hash = transaction.signature_hash();
            sign_message(B256::from_slice(secret.as_ref()), hash)?
        }
        _ => return Err(eyre!("missing signature({:?}) or secret({:?})", tx.signature, secret)),
    };
    Ok(TransactionSigned::from_transaction_and_signature(transaction, signature))
}
