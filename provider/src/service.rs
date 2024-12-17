use async_trait::async_trait;
use axum::extract::*;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use axum_extra::extract::CookieJar;
use http::header;
use http::Method;
use http::StatusCode;
use near_crypto::PublicKey;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::error;
use tracing::info;

use crate::AccountInfoPublic;
use crate::ProviderCtx;
use crate::ProviderDb;
use crate::{ModelInfo, Provider, ProviderConfig, BAD_REQUEST, FOUR_HUNDRED};
use openaiapi::apis::completions::{
    Completions, CreateCompletionResponse as CreateCompletionResponseAPI,
};
use openaiapi::apis::models::{
    DeleteModelResponse, ListModelsResponse, Models, RetrieveModelResponse,
};
use openaiapi::models::{
    self, CreateCompletionRequest as CreateCompletionRequestAPI, DeleteModelPathParams, Error,
    RetrieveModelPathParams,
};

use openaiclient::apis::completions_api::create_completion;
use openaiclient::apis::configuration::Configuration;
use openaiclient::models::{
    CreateCompletionRequest as CreateCompletionRequestClient,
    CreateCompletionResponse as CreateCompletionResponseClient,
};

use near_crypto::{PublicKey as NearPublicKey, Signature as NearSignature};
use near_primitives::types::AccountId;
use near_primitives::types::BlockReference;
use near_sdk::json_types::U128;

// Reminder: this is private information, do not expose or serialize this struct

#[derive(Debug)]
pub struct ProviderBaseServiceError {
    pub message: String,
    pub status_code: StatusCode,
}

impl IntoResponse for ProviderBaseServiceError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status_code,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            Json(json!({ "message": self.message })),
        )
            .into_response()
    }
}

#[derive(Clone)]
pub struct ProviderBaseService {
    ctx: ProviderCtx,
}

impl AsRef<ProviderBaseService> for ProviderBaseService {
    fn as_ref(&self) -> &ProviderBaseService {
        self
    }
}

impl ProviderBaseService {
    pub fn new(ctx: ProviderCtx) -> Self {
        info!("Creating ProviderBaseService");
        Self { ctx }
    }

    pub fn router(self) -> axum::Router {
        Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/info", get(info_handler))
            .route("/pc/state/:channel_name", get(get_pc_state))
            .route("/pc/state/:channel_name", post(update_pc_state))
            .with_state(self)
    }
}

async fn info_handler(State(state): State<ProviderBaseService>) -> Json<AccountInfoPublic> {
    Json(state.ctx.public_account_info().await)
}

async fn get_pc_state(
    State(state): State<ProviderBaseService>,
    Path(channel_name): Path<String>,
) -> impl IntoResponse {
    state
        .ctx
        .get_pc_state(&channel_name)
        .await
        .map_err(|e| {
            error!("Unable to get payment channel state: {:?}", e);
            ProviderBaseServiceError {
                message: "Unable to get payment channel state".to_string(),
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            }
        })
        .map(|value| {
            value
                .ok_or(ProviderBaseServiceError {
                    message: "Payment channel not found".to_string(),
                    status_code: StatusCode::NOT_FOUND,
                })
                .map(|value| (StatusCode::OK, Json(value)))
        })
}

#[derive(Deserialize)]
pub struct UpdatePcStateRequest {
    spent_balance: U128, // in yoctoNEAR
    signature: String,
}

async fn update_pc_state(
    State(state): State<ProviderBaseService>,
    Path(sender_account_id): Path<AccountId>,
    Json(body): Json<UpdatePcStateRequest>,
) -> impl IntoResponse {
    // Parse the signature
    let signature = match NearSignature::from_str(&body.signature) {
        Ok(signature) => signature,
        Err(_) => {
            return (ProviderBaseServiceError {
                message: format!(
                    "Invalid signature format. Please provide a b58 encoded signature with a valid curve type (ed25519 or secp256k1)"
                ),
                status_code: StatusCode::BAD_REQUEST,
            })
            .into_response();
        }
    };

    // Get the channel state

    // Get the account info, check if the account is valid
    // and get all the public keys
    // let rpc = state.ctx.near_network_config.json_rpc_client();
    // let query_view_method_request = near_jsonrpc_client::methods::query::RpcQueryRequest {
    //     block_reference: BlockReference::latest(),
    //     request: near_primitives::views::QueryRequest::ViewAccessKeyList {
    //         account_id: sender_account_id.clone(),
    //     },
    // };
    // let account_info = match rpc.call(query_view_method_request).await {
    //     Ok(rpc_response) => match rpc_response.access_key_list_view() {
    //         Ok(access_key_list) => access_key_list,
    //         Err(e) => {
    //             error!("Unable to verify account {}: {:?}", sender_account_id, e);
    //             return (ProviderBaseServiceError {
    //                 message: format!("Unable to verify account {}", sender_account_id),
    //                 status_code: StatusCode::BAD_REQUEST,
    //             })
    //             .into_response();
    //         }
    //     },
    //     Err(e) => {
    //         error!("Unable to verify account {}: {:?}", sender_account_id, e);
    //         return (ProviderBaseServiceError {
    //             message: format!(
    //                 "Unable to verify account {} due to rpc error",
    //                 sender_account_id
    //             ),
    //             status_code: StatusCode::BAD_REQUEST,
    //         })
    //         .into_response();
    //     }
    // };

    // signed payload
    // comma seperated string of the following:
    // (channel_id, sender_account_id, spent_balance)
    // let channel_name = body.channel_name.clone();
    // let spent_balance = serde_json::to_string(&body.spent_balance.clone()).unwrap();
    // let payload_raw = vec![channel_name, spent_balance];
    // let payload = payload_raw.join(",");
    // let payload_bytes = payload.as_bytes();

    // println!("payload: {}", payload);
    // println!("signature: {}", signature);
    // println!("account_info: {:?}", account_info);

    // // first byte contains CurveType, so we're removing it
    // let verified_pk = match account_info
    //     .keys
    //     .into_iter()
    //     .map(|key| key.public_key)
    //     .find(|pk| signature.verify(payload_bytes, pk))
    // {
    //     Some(pk) => pk,
    //     None => {
    //         return (ProviderBaseServiceError {
    //             message: format!(
    //                 "Bad signature. Cannot find valid public key for {} to verify the payload",
    //                 sender_account_id
    //             ),
    //             status_code: StatusCode::BAD_REQUEST,
    //         })
    //         .into_response();
    //     }
    // };

    (StatusCode::OK, Json("OK")).into_response()
}

#[derive(Clone)]
pub struct ProviderOaiService {
    ctx: ProviderCtx,
}

impl ProviderOaiService {
    pub fn new(ctx: ProviderCtx) -> Self {
        info!("Creating ProviderOaiService");
        info!(
            "Available providers: {:?}",
            ctx.config
                .providers
                .iter()
                .map(|p| p.canonical_name.clone())
                .collect::<Vec<_>>()
        );
        Self { ctx }
    }
}

impl AsRef<ProviderOaiService> for ProviderOaiService {
    fn as_ref(&self) -> &ProviderOaiService {
        self
    }
}

#[async_trait]
impl Models for ProviderOaiService {
    /// Delete a fine-tuned model. You must have the Owner role in your organization to delete a model..
    ///
    /// DeleteModel - DELETE /oai/models/{model}
    async fn delete_model(
        &self,
        _method: Method,
        _host: Host,
        _cookies: CookieJar,
        _path_params: DeleteModelPathParams,
    ) -> Result<DeleteModelResponse, ()> {
        Ok(DeleteModelResponse::Status500_InternalServerError(
            Error::new(
                "not_implemented".to_string(),
                "Not implemented".to_string(),
                "None".to_string(),
                "invalid_request_error".to_string(),
            ),
        ))
    }

    /// Lists the currently available models, and provides basic information about each one such as the owner and availability..
    ///
    /// ListModels - GET /oai/models
    async fn list_models(
        &self,
        _method: Method,
        _host: Host,
        _cookies: CookieJar,
    ) -> Result<ListModelsResponse, ()> {
        Ok(ListModelsResponse::Status500_InternalServerError(
            Error::new(
                "not_implemented".to_string(),
                "Not implemented".to_string(),
                "None".to_string(),
                "invalid_request_error".to_string(),
            ),
        ))
    }

    /// Retrieves a model instance, providing basic information about the model such as the owner and permissioning..
    ///
    /// RetrieveModel - GET /oai/models/{model}
    async fn retrieve_model(
        &self,
        _method: Method,
        _host: Host,
        _cookies: CookieJar,
        _path_params: RetrieveModelPathParams,
    ) -> Result<RetrieveModelResponse, ()> {
        Ok(RetrieveModelResponse::Status500_InternalServerError(
            Error::new(
                "not_implemented".to_string(),
                "Not implemented".to_string(),
                "None".to_string(),
                "invalid_request_error".to_string(),
            ),
        ))
    }
}

#[async_trait]
impl Completions for ProviderOaiService {
    async fn create_completion(
        &self,
        _method: Method,
        _host: Host,
        _cookies: CookieJar,
        mut body: CreateCompletionRequestAPI,
    ) -> Result<CreateCompletionResponseAPI, ()> {
        // Parse the model info from the request
        let model_info: ModelInfo = match ModelInfo::from_str(&body.model) {
            Ok(m) => m,
            Err(e) => {
                return Ok(CreateCompletionResponseAPI::Status400_BadRequest(
                    Error::new(
                        FOUR_HUNDRED.to_string(),
                        BAD_REQUEST.to_string(),
                        e.to_string(),
                        "".to_string(),
                    ),
                ));
            }
        };

        // Get the provider from the config
        let provider: &Provider = match self
            .ctx
            .config
            .providers
            .iter()
            .find(|p| p.canonical_name == model_info.provider)
        {
            Some(p) => p,
            None => {
                return Ok(CreateCompletionResponseAPI::Status400_BadRequest(
                    Error::new(
                        FOUR_HUNDRED.to_string(),
                        BAD_REQUEST.to_string(),
                        format!("Provider {} not found", model_info.provider),
                        "".to_string(),
                    ),
                ))
            }
        };

        // Create the configuration from the provider configuration
        let mut configuration: Configuration = Configuration::new();
        configuration.user_agent = None;
        configuration.base_path = provider.url.clone();
        configuration.bearer_access_token = Some(provider.api_key.clone());

        // Convert the user request to a client request
        // by serialize -> deserialize chain
        body.model = model_info.model_name;
        let serialized_body = serde_json::to_string(&body).unwrap();
        let client_request: CreateCompletionRequestClient =
            serde_json::from_str(&serialized_body).unwrap();

        let response = create_completion(&configuration, client_request).await;
        match response {
            Ok(response) => {
                let serialized_response = serde_json::to_string(&response).unwrap();
                let api_response: models::CreateCompletionResponse =
                    serde_json::from_str(&serialized_response).unwrap();
                return Ok(CreateCompletionResponseAPI::Status200_OK(api_response));
            }
            Err(e) => Ok(CreateCompletionResponseAPI::Status500_InternalServerError(
                Error::new(
                    "Internal Server Error".to_string(),
                    "Internal Server Error".to_string(),
                    e.to_string(),
                    "invalid_request_error".to_string(),
                ),
            )),
        }
    }
}
