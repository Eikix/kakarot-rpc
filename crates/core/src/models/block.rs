use async_trait::async_trait;
use reth_primitives::{BlockId as EthereumBlockId, BlockNumberOrTag, Bloom, Bytes, H256, H64, U256};
use reth_rpc_types::{Block, BlockTransactions, Header, RichBlock};
use starknet::core::types::{
    BlockId as StarknetBlockId, BlockTag, FieldElement, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    Transaction,
};
use starknet::providers::jsonrpc::JsonRpcTransport;

use super::convertible::ConvertibleStarknetBlock;
use super::felt::Felt252Wrapper;
use crate::client::api::KakarotEthApi;
use crate::client::constants::{
    DIFFICULTY, EARLIEST_BLOCK_NUMBER, GAS_LIMIT, GAS_USED, MIX_HASH, NONCE, SIZE, TOTAL_DIFFICULTY,
};
use crate::client::errors::EthApiError;

pub struct EthBlockId(EthereumBlockId);

impl EthBlockId {
    pub fn new(block_id: EthereumBlockId) -> Self {
        Self(block_id)
    }

    pub fn to_starknet_block_id<T: JsonRpcTransport>(&self) -> Result<StarknetBlockId, EthApiError<T::Error>> {
        match self.0 {
            EthereumBlockId::Hash(hash) => {
                let hash: Felt252Wrapper = hash.block_hash.try_into()?;
                Ok(StarknetBlockId::Hash(hash.into()))
            }
            EthereumBlockId::Number(block_number_or_tag) => match block_number_or_tag {
                BlockNumberOrTag::Safe | BlockNumberOrTag::Latest | BlockNumberOrTag::Finalized => {
                    Ok(StarknetBlockId::Tag(BlockTag::Latest))
                }
                BlockNumberOrTag::Earliest => Ok(StarknetBlockId::Number(EARLIEST_BLOCK_NUMBER)),
                BlockNumberOrTag::Pending => Ok(StarknetBlockId::Tag(BlockTag::Pending)),
                BlockNumberOrTag::Number(number) => Ok(StarknetBlockId::Number(number)),
            },
        }
    }
}

impl From<EthBlockId> for EthereumBlockId {
    fn from(eth_block_id: EthBlockId) -> Self {
        eth_block_id.0
    }
}

/// Implement getters for fields that are present in Starknet Blocks, both in pending and validated
/// state. For example, `parent_hash` is present in both `PendingBlock` and `Block`.
macro_rules! implement_starknet_block_getters {
    ($(($enum:ty, $field:ident, $field_type:ty)),*) => {
        $(pub fn $field(&self) -> $field_type {
            match &self.0 {
                <$enum>::PendingBlock(pending_block_with_tx_hashes) => {
                    pending_block_with_tx_hashes.$field.clone()
                }
                <$enum>::Block(block_with_tx_hashes) => {
                    block_with_tx_hashes.$field.clone()
                }
            }
        })*
    };
}

/// Implement getters for fields that are only present in Starknet Blocks that are not pending.
/// For example, `block_hash` is only present in `Block` and not in `PendingBlock`.
macro_rules! implement_starknet_block_getters_not_pending {
    ($(($enum:ty, $field:ident, $field_type:ty)),*) => {
        $(pub fn $field(&self) -> Option<$field_type> {
            match &self.0 {
                <$enum>::PendingBlock(_) => {
                    None
                }
                <$enum>::Block(block_with_txs) => {
                    Some(block_with_txs.$field.clone())
                }
            }
        })*
    };
}

pub struct BlockWithTxHashes(MaybePendingBlockWithTxHashes);

impl BlockWithTxHashes {
    pub fn new(block: MaybePendingBlockWithTxHashes) -> Self {
        Self(block)
    }

    implement_starknet_block_getters!(
        (MaybePendingBlockWithTxHashes, parent_hash, FieldElement),
        (MaybePendingBlockWithTxHashes, sequencer_address, FieldElement),
        (MaybePendingBlockWithTxHashes, timestamp, u64),
        (MaybePendingBlockWithTxHashes, transactions, Vec<FieldElement>)
    );

    implement_starknet_block_getters_not_pending!(
        (MaybePendingBlockWithTxHashes, block_hash, FieldElement),
        (MaybePendingBlockWithTxHashes, block_number, u64)
    );
}

pub struct BlockWithTxs(MaybePendingBlockWithTxs);

impl BlockWithTxs {
    pub fn new(block: MaybePendingBlockWithTxs) -> Self {
        Self(block)
    }

    implement_starknet_block_getters!(
        (MaybePendingBlockWithTxs, parent_hash, FieldElement),
        (MaybePendingBlockWithTxs, sequencer_address, FieldElement),
        (MaybePendingBlockWithTxs, timestamp, u64),
        (MaybePendingBlockWithTxs, transactions, Vec<Transaction>)
    );

    implement_starknet_block_getters_not_pending!(
        (MaybePendingBlockWithTxs, block_hash, FieldElement),
        (MaybePendingBlockWithTxs, block_number, u64)
    );
}

#[async_trait]
impl ConvertibleStarknetBlock for BlockWithTxHashes {
    async fn to_eth_block<T: JsonRpcTransport>(&self, client: &dyn KakarotEthApi<T>) -> RichBlock {
        // TODO: Fetch real data
        let gas_limit = *GAS_LIMIT;

        // TODO: Fetch real data
        let gas_used = *GAS_USED;

        // TODO: Fetch real data
        let difficulty = *DIFFICULTY;

        // TODO: Fetch real data
        let nonce: Option<H64> = Some(H64::zero());

        // TODO: Fetch real data
        let size: Option<U256> = *SIZE;

        // Bloom is a byte array of length 256
        let logs_bloom = Bloom::default();
        let extra_data = Bytes::from(b"0x00");

        // TODO: Fetch real data
        let base_fee_per_gas = client.base_fee_per_gas();
        // TODO: Fetch real data
        let mix_hash = *MIX_HASH;

        let parent_hash = H256::from_slice(&self.parent_hash().to_bytes_be());
        let sequencer = Felt252Wrapper::from(self.sequencer_address()).troncate_to_ethereum_address();
        let timestamp = U256::from(self.timestamp());

        let hash = self.block_hash().as_ref().map(|hash| H256::from_slice(&hash.to_bytes_be()));
        let number = self.block_number().map(U256::from);

        // TODO: Add filter to tx_hashes
        let transactions = BlockTransactions::Hashes(
            self.transactions().iter().map(|tx| H256::from_slice(&tx.to_bytes_be())).collect(),
        );

        let header = Header {
            // PendingBlockWithTxHashes doesn't have a block hash
            hash,
            parent_hash,
            uncles_hash: parent_hash,
            miner: sequencer,
            // PendingBlockWithTxHashes doesn't have a state root
            state_root: H256::zero(),
            // PendingBlockWithTxHashes doesn't have a transactions root
            transactions_root: H256::zero(),
            // PendingBlockWithTxHashes doesn't have a receipts root
            receipts_root: H256::zero(),
            // PendingBlockWithTxHashes doesn't have a block number
            number,
            gas_used,
            gas_limit,
            extra_data,
            logs_bloom,
            timestamp,
            difficulty,
            nonce,
            base_fee_per_gas: Some(base_fee_per_gas),
            mix_hash,
            withdrawals_root: Some(H256::zero()),
        };
        let block = Block {
            header,
            total_difficulty: *TOTAL_DIFFICULTY,
            uncles: vec![],
            transactions,
            size,
            withdrawals: Some(vec![]),
        };
        block.into()
    }
}

#[async_trait]
impl ConvertibleStarknetBlock for BlockWithTxs {
    async fn to_eth_block<T: JsonRpcTransport>(&self, client: &dyn KakarotEthApi<T>) -> RichBlock {
        // TODO: Fetch real data
        let gas_limit = *GAS_LIMIT;

        // TODO: Fetch real data
        let gas_used = *GAS_USED;

        // TODO: Fetch real data
        let difficulty = *DIFFICULTY;

        // TODO: Fetch real data
        let nonce: Option<H64> = *NONCE;

        // TODO: Fetch real data
        let size: Option<U256> = *SIZE;

        // Bloom is a byte array of length 256
        let logs_bloom = Bloom::default();
        let extra_data: Bytes = Bytes::from(b"0x00");

        // TODO: Fetch real data
        let base_fee_per_gas = client.base_fee_per_gas();
        // TODO: Fetch real data
        let mix_hash = *MIX_HASH;

        let parent_hash = H256::from_slice(&self.parent_hash().to_bytes_be());

        let sequencer = Felt252Wrapper::from(self.sequencer_address()).troncate_to_ethereum_address();

        let timestamp = U256::from(self.timestamp());

        let hash = self.block_hash().as_ref().map(|hash| H256::from_slice(&hash.to_bytes_be()));
        let number = self.block_number().map(U256::from);

        let transactions = client.filter_starknet_into_eth_txs(self.transactions().into(), hash, number).await;
        let header = Header {
            // PendingBlockWithTxs doesn't have a block hash
            hash,
            parent_hash,
            uncles_hash: parent_hash,
            miner: sequencer,
            // PendingBlockWithTxs doesn't have a state root
            state_root: H256::zero(),
            // PendingBlockWithTxs doesn't have a transactions root
            transactions_root: H256::zero(),
            // PendingBlockWithTxs doesn't have a receipts root
            receipts_root: H256::zero(),
            // PendingBlockWithTxs doesn't have a block number
            number,
            gas_used,
            gas_limit,
            extra_data,
            logs_bloom,
            timestamp,
            difficulty,
            nonce,
            base_fee_per_gas: Some(base_fee_per_gas),
            mix_hash,
            withdrawals_root: Some(H256::zero()),
        };
        let block = Block {
            header,
            total_difficulty: *TOTAL_DIFFICULTY,
            uncles: vec![],
            transactions,
            size,
            withdrawals: Some(vec![]),
        };
        block.into()
    }
}