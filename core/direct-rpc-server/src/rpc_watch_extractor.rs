/*
	Copyright 2021 Integritee AG and Supercomputing Systems AG

	Licensed under the Apache License, Version 2.0 (the "License");
	you may not use this file except in compliance with the License.
	You may obtain a copy of the License at

		http://www.apache.org/licenses/LICENSE-2.0

	Unless required by applicable law or agreed to in writing, software
	distributed under the License is distributed on an "AS IS" BASIS,
	WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
	See the License for the specific language governing permissions and
	limitations under the License.

*/

use crate::{DetermineWatch, DirectRpcError, DirectRpcResult, RpcHash};
use alloc::format;
use codec::Decode;
use itp_rpc::{RpcResponse, RpcReturnValue};
use itp_types::DirectRequestStatus;
use itp_utils::FromHexPrefixed;
use log::debug;
use std::marker::PhantomData;

pub struct RpcWatchExtractor<Hash>
where
	Hash: RpcHash,
{
	phantom_data: PhantomData<Hash>,
}

impl<Hash> RpcWatchExtractor<Hash>
where
	Hash: RpcHash,
{
	pub fn new() -> Self {
		Self::default()
	}
}

impl<Hash> Default for RpcWatchExtractor<Hash>
where
	Hash: RpcHash,
{
	fn default() -> Self {
		RpcWatchExtractor { phantom_data: PhantomData }
	}
}

impl<Hash> DetermineWatch for RpcWatchExtractor<Hash>
where
	Hash: RpcHash + Decode,
{
	type Hash = Hash;

	fn must_be_watched(&self, rpc_response: &RpcResponse) -> DirectRpcResult<Option<Self::Hash>> {
		let rpc_return_value = match RpcReturnValue::from_hex(&rpc_response.result) {
			Ok(return_value) => return_value,
			Err(e) => {
				// `author_submitAndWatchExtrinsic` does currently only return the top hash
				// as the first subscription response in order to comply with JSON RPC 2.0.
				//
				// We support this for now with this hack here, but it should be properly
				// refactored in #1624.
				if let Ok(hash) = Self::Hash::from_hex(&rpc_response.result) {
					// fixme: fix hack in #1624.
					debug!("returning hash as connection token: {hash:?}");
					return Ok(Some(hash))
				}

				return Err(DirectRpcError::Other(format!("{:?}", e).into()))
			},
		};

		if !rpc_return_value.do_watch {
			return Ok(None)
		}

		match rpc_return_value.status {
			DirectRequestStatus::TrustedOperationStatus(_) =>
				Self::Hash::decode(&mut rpc_return_value.value.as_slice())
					.map(Some)
					.map_err(DirectRpcError::EncodingError),
			_ => Ok(None),
		}
	}
}

#[cfg(test)]
pub mod tests {

	use super::*;
	use crate::builders::{
		rpc_response_builder::RpcResponseBuilder, rpc_return_value_builder::RpcReturnValueBuilder,
	};
	use codec::Encode;
	use itp_rpc::Id;
	use itp_types::TrustedOperationStatus;

	#[test]
	fn invalid_rpc_response_returns_error() {
		let watch_extractor = RpcWatchExtractor::<String>::new();
		let rpc_response = RpcResponse {
			id: Id::Number(1u32),
			jsonrpc: String::from("json"),
			result: "hello".to_string(),
		};

		assert!(watch_extractor.must_be_watched(&rpc_response).is_err());
	}

	#[test]
	fn rpc_response_without_watch_flag_must_not_be_watched() {
		let watch_extractor = RpcWatchExtractor::<String>::new();
		let rpc_result = RpcReturnValueBuilder::new()
			.with_do_watch(false)
			.with_status(DirectRequestStatus::TrustedOperationStatus(TrustedOperationStatus::Ready))
			.build();
		let rpc_response = RpcResponseBuilder::new().with_result(rpc_result).build();

		let do_watch = watch_extractor.must_be_watched(&rpc_response).unwrap();

		assert_eq!(None, do_watch);
	}

	#[test]
	fn rpc_response_with_watch_flag_must_be_watched() {
		let hash = String::from("rpc_hash");
		let watch_extractor = RpcWatchExtractor::<String>::new();
		let rpc_return_value = RpcReturnValueBuilder::new()
			.with_do_watch(true)
			.with_value(hash.encode())
			.with_status(DirectRequestStatus::TrustedOperationStatus(TrustedOperationStatus::Ready))
			.build();
		let rpc_response = RpcResponseBuilder::new().with_result(rpc_return_value).build();

		let do_watch = watch_extractor.must_be_watched(&rpc_response).unwrap();

		assert_eq!(Some(hash.clone()), do_watch);
	}
}
