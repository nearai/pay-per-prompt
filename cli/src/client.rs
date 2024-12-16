use near_crypto::InMemorySigner;
use near_jsonrpc_client::{
    methods::{self, tx::RpcTransactionResponse},
    JsonRpcClient,
};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::{
    types::{AccountId, BlockReference, Finality, FunctionArgs},
    views::QueryRequest,
};
use near_sdk::{Gas, NearToken};
use serde::de::DeserializeOwned;
use serde_json::from_slice;

#[derive(Clone)]
pub struct Client {
    client: JsonRpcClient,
    verbose: bool,
}

impl Client {
    pub fn new(server_addr: &str, verbose: bool) -> Self {
        Self {
            client: JsonRpcClient::connect(server_addr),
            verbose,
        }
    }

    pub async fn view_call<R: DeserializeOwned>(
        &self,
        account_id: AccountId,
        method_name: impl ToString,
        args: impl ToString,
    ) -> R {
        let request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::CallFunction {
                account_id: account_id.into(),
                method_name: method_name.to_string(),
                args: FunctionArgs::from(args.to_string().into_bytes()),
            },
        };

        let result = self.client.call(request).await.unwrap();

        match result.kind {
            QueryResponseKind::CallResult(result) => from_slice::<R>(&result.result).unwrap(),
            _ => unreachable!(),
        }
    }

    pub async fn change_call(
        &self,
        signer: &InMemorySigner,
        contract: AccountId,
        method_name: impl ToString,
        args: impl ToString,
        gas: Gas,
        deposit: NearToken,
    ) -> RpcTransactionResponse {
        let access_key_query_response = self
            .client
            .call(near_jsonrpc_client::methods::query::RpcQueryRequest {
                block_reference: near_primitives::types::BlockReference::latest(),
                request: near_primitives::views::QueryRequest::ViewAccessKey {
                    account_id: signer.account_id.clone(),
                    public_key: signer.public_key.clone(),
                },
            })
            .await
            .unwrap();

        let current_nonce = match access_key_query_response.kind {
            QueryResponseKind::AccessKey(access_key) => access_key.nonce,
            _ => unreachable!(),
        };

        let transaction = near_primitives::transaction::TransactionV0 {
            signer_id: signer.account_id.clone(),
            public_key: signer.public_key.clone(),
            nonce: current_nonce + 1,
            receiver_id: contract,
            block_hash: access_key_query_response.block_hash,
            actions: vec![near_primitives::action::Action::FunctionCall(Box::new(
                near_primitives::action::FunctionCallAction {
                    method_name: method_name.to_string(),
                    args: args.to_string().into_bytes(),
                    gas: gas.as_gas(),
                    deposit: deposit.as_yoctonear(),
                },
            ))],
        };

        let request =
            near_jsonrpc_client::methods::broadcast_tx_async::RpcBroadcastTxAsyncRequest {
                signed_transaction: near_primitives::transaction::Transaction::V0(transaction)
                    .sign(&near_crypto::Signer::InMemory(signer.clone())),
            };

        let sent_at = tokio::time::Instant::now();
        let tx_hash = self.client.call(request).await.unwrap();

        if self.verbose {
            eprintln!(
                "\nSubmitted transaction.\nhttps://nearblocks.io/txns/{:?}\n",
                tx_hash
            );
        }

        loop {
            let response = self
                .client
                .call(methods::tx::RpcTransactionStatusRequest {
                    transaction_info: methods::tx::TransactionInfo::TransactionId {
                        tx_hash,
                        sender_account_id: signer.account_id.clone(),
                    },
                    wait_until: near_primitives::views::TxExecutionStatus::Executed,
                })
                .await;
            let received_at = tokio::time::Instant::now();
            let delta = (received_at - sent_at).as_secs();

            assert!(delta <= 60);

            match response {
                Err(err) => match err.handler_error() {
                    Some(
                        methods::tx::RpcTransactionError::TimeoutError
                        | methods::tx::RpcTransactionError::UnknownTransaction { .. },
                    ) => {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        continue;
                    }
                    _ => unreachable!(),
                },
                Ok(response) => return response,
            }
        }
    }
}
