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
use serde::Deserialize;
use serde_json::json;
use std::str::FromStr;
use tracing::error;
use tracing::info;

use crate::AccountInfoPublic;
use crate::ProviderCtx;
use crate::SignedState;
use crate::{ModelInfo, Provider, BAD_REQUEST, FOUR_HUNDRED};
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
use openaiclient::models::CreateCompletionRequest as CreateCompletionRequestClient;

use near_crypto::Signature as NearSignature;
use near_primitives::types::AccountId;
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
            .route("/pc/state", post(post_pc_signed_state))
            .with_state(self)
    }
}

async fn info_handler(State(state): State<ProviderBaseService>) -> Json<AccountInfoPublic> {
    Json(state.ctx.public_account_info().await)
}

async fn get_pc_state(
    State(state): State<ProviderBaseService>,
    Path(channel_name): Path<String>,
) -> Result<impl IntoResponse, ProviderBaseServiceError> {
    let result = state
        .ctx
        .get_pc_state(&channel_name)
        .await
        .map_err(|e| {
            error!("Unable to get payment channel state: {:?}", e);
            ProviderBaseServiceError {
                message: "Unable to get payment channel state".to_string(),
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?
        .ok_or(ProviderBaseServiceError {
            message: "Payment channel not found".to_string(),
            status_code: StatusCode::NOT_FOUND,
        })?;

    Ok((StatusCode::OK, Json(result)))
}

async fn post_pc_signed_state(
    State(state): State<ProviderBaseService>,
    Json(signed_state): Json<SignedState>,
) -> Result<impl IntoResponse, ProviderBaseServiceError> {
    state
        .ctx
        .validate_insert_signed_state(&signed_state)
        .await
        .map_err(|e| {
            error!("Unable to validate signed state: {:?}", e);
            ProviderBaseServiceError {
                message: "Unable to validate signed state".to_string(),
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    Ok((StatusCode::CREATED, Json(signed_state)))
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
