// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Receipt

use bigint::prelude::U256;
use bigint::hash::{H256, H2048};
use util::Address;
use heapsize::HeapSizeOf;
use rlp::*;
use ethbloom::{Bloom, BloomRef};

use {BlockNumber};
use log_entry::{LogEntry, LocalizedLogEntry};

/// Transaction outcome store in the receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOutcome {
	/// Status and state root are unknown under EIP-98 rules.
	Unknown,
	/// State root is known. Pre EIP-98 and EIP-658 rules.
	StateRoot(H256),
	/// Status code is known. EIP-658 rules.
	StatusCode(u8),
}

/// Information describing execution of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
	/// The total gas used in the block following execution of the transaction.
	pub gas_used: U256,
	/// The OR-wide combination of all logs' blooms for this transaction.
	pub log_bloom: Bloom,
	/// The logs stemming from this transaction.
	pub logs: Vec<LogEntry>,
	/// Transaction outcome.
	pub outcome: TransactionOutcome,
}

impl Receipt {
	/// Create a new receipt.
	pub fn new(outcome: TransactionOutcome, gas_used: U256, logs: Vec<LogEntry>) -> Receipt {
		Receipt {
			gas_used: gas_used,
			log_bloom: logs.iter().fold(Bloom::default(), |mut b, l| {
                b.accrue_bloom(BloomRef::from(&l.bloom()));
                b
            }),
			logs: logs,
			outcome: outcome,
		}
	}
}

// Wrapper struct used for RLP encoding and decoding
#[derive(Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
struct RlpBloom(H2048);

impl Encodable for Receipt {
	fn rlp_append(&self, s: &mut RlpStream) {
		match self.outcome {
			TransactionOutcome::Unknown => {
				s.begin_list(3);
			},
			TransactionOutcome::StateRoot(ref root) => {
				s.begin_list(4);
				s.append(root);
			},
			TransactionOutcome::StatusCode(ref status_code) => {
				s.begin_list(4);
				s.append(status_code);
			},
		}
        let rlp_bloom = RlpBloom(H2048::from(self.log_bloom.data().to_owned()));
		s.append(&self.gas_used);
		s.append(&rlp_bloom);
		s.append_list(&self.logs);
	}
}

impl Decodable for Receipt {
	fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
		if rlp.item_count()? == 3 {
            let rlp_bloom: RlpBloom = match rlp.val_at(1) {
                Ok(bloom) => bloom,
                Err(_) => return Err(DecoderError::Custom("No bloom."))
            };
            let rlp_bloom = rlp_bloom.0.to_vec();
            let mut rlp_bloom_limited_slice = [0; 256];
            rlp_bloom_limited_slice.clone_from_slice(rlp_bloom.as_slice());
            let rlp_bloom = Bloom::from(rlp_bloom_limited_slice);

            let receipt = Receipt {
				outcome: TransactionOutcome::Unknown,
				gas_used: rlp.val_at(0)?,
				log_bloom: rlp_bloom,
				logs: rlp.list_at(2)?,
			};
			Ok(receipt)
		} else {
            let rlp_bloom: RlpBloom = match rlp.val_at(2) {
                Ok(bloom) => bloom,
                Err(_) => return Err(DecoderError::Custom("No bloom."))
            };
            let rlp_bloom = rlp_bloom.0.to_vec();
            let mut rlp_bloom_limited_slice = [0; 256];
            rlp_bloom_limited_slice.clone_from_slice(rlp_bloom.as_slice());
            let rlp_bloom = Bloom::from(rlp_bloom_limited_slice);

			let receipt = Receipt {
				gas_used: rlp.val_at(1)?,
				log_bloom: rlp_bloom,
				logs: rlp.list_at(3)?,
				outcome: {
					let first = rlp.at(0)?;
					if first.is_data() && first.data()?.len() <= 1 {
						TransactionOutcome::StatusCode(first.as_val()?)
					} else {
						TransactionOutcome::StateRoot(first.as_val()?)
					}
				}
			};
            Ok(receipt)
		}
	}
}

impl HeapSizeOf for Receipt {
	fn heap_size_of_children(&self) -> usize {
		self.logs.heap_size_of_children()
	}
}

/// Receipt with additional info.
#[derive(Debug, Clone, PartialEq)]
pub struct RichReceipt {
	/// Transaction hash.
	pub transaction_hash: H256,
	/// Transaction index.
	pub transaction_index: usize,
	/// The total gas used in the block following execution of the transaction.
	pub cumulative_gas_used: U256,
	/// The gas used in the execution of the transaction. Note the difference of meaning to `Receipt::gas_used`.
	pub gas_used: U256,
	/// Contract address.
	pub contract_address: Option<Address>,
	/// Logs
	pub logs: Vec<LogEntry>,
	/// Logs bloom
	pub log_bloom: Bloom,
	/// Transaction outcome.
	pub outcome: TransactionOutcome,
}

/// Receipt with additional info.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalizedReceipt {
	/// Transaction hash.
	pub transaction_hash: H256,
	/// Transaction index.
	pub transaction_index: usize,
	/// Block hash.
	pub block_hash: H256,
	/// Block number.
	pub block_number: BlockNumber,
	/// The total gas used in the block following execution of the transaction.
	pub cumulative_gas_used: U256,
	/// The gas used in the execution of the transaction. Note the difference of meaning to `Receipt::gas_used`.
	pub gas_used: U256,
	/// Contract address.
	pub contract_address: Option<Address>,
	/// Logs
	pub logs: Vec<LocalizedLogEntry>,
	/// Logs bloom
	pub log_bloom: Bloom,
	/// Transaction outcome.
	pub outcome: TransactionOutcome,
}

#[cfg(test)]
mod tests {
	use super::{Receipt, TransactionOutcome};
	use log_entry::LogEntry;

	#[test]
	fn test_no_state_root() {
		let expected = ::rustc_hex::FromHex::from_hex("f9014183040caeb9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000f838f794dcf421d093428b096ca501a7cd1a740855a7976fc0a00000000000000000000000000000000000000000000000000000000000000000").unwrap();
		let r = Receipt::new(
			TransactionOutcome::Unknown,
			0x40cae.into(),
			vec![LogEntry {
				address: "dcf421d093428b096ca501a7cd1a740855a7976f".into(),
				topics: vec![],
				data: vec![0u8; 32]
			}]
		);
		assert_eq!(&::rlp::encode(&r)[..], &expected[..]);
	}

	#[test]
	fn test_basic() {
		let expected = ::rustc_hex::FromHex::from_hex("f90162a02f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee83040caeb9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000f838f794dcf421d093428b096ca501a7cd1a740855a7976fc0a00000000000000000000000000000000000000000000000000000000000000000").unwrap();
		let r = Receipt::new(
			TransactionOutcome::StateRoot("2f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee".into()),
			0x40cae.into(),
			vec![LogEntry {
				address: "dcf421d093428b096ca501a7cd1a740855a7976f".into(),
				topics: vec![],
				data: vec![0u8; 32]
			}]
		);
		let encoded = ::rlp::encode(&r);
		assert_eq!(&encoded[..], &expected[..]);
		let decoded: Receipt = ::rlp::decode(&encoded);
		assert_eq!(decoded, r);
	}

	#[test]
	fn test_status_code() {
		let expected = ::rustc_hex::FromHex::from_hex("f901428083040caeb9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000f838f794dcf421d093428b096ca501a7cd1a740855a7976fc0a00000000000000000000000000000000000000000000000000000000000000000").unwrap();
		let r = Receipt::new(
			TransactionOutcome::StatusCode(0),
			0x40cae.into(),
			vec![LogEntry {
				address: "dcf421d093428b096ca501a7cd1a740855a7976f".into(),
				topics: vec![],
				data: vec![0u8; 32]
			}]
		);
		let encoded = ::rlp::encode(&r);
		assert_eq!(&encoded[..], &expected[..]);
		let decoded: Receipt = ::rlp::decode(&encoded);
		assert_eq!(decoded, r);
	}
}
