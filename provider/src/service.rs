use async_trait::async_trait;
use axum::extract::*;
use axum::routing::get;
use axum::Router;
use axum_extra::extract::CookieJar;
use http::Method;
use tracing::info;

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
use openaiclient::apis::configuration::{ApiKey, Configuration};
use openaiclient::models::{
    CreateCompletionRequest as CreateCompletionRequestClient,
    CreateCompletionResponse as CreateCompletionResponseClient,
};

#[derive(Clone)]
pub struct ProviderBaseService {}

impl ProviderBaseService {
    pub fn new() -> Self {
        info!("Creating ProviderBaseService");
        Self {}
    }
    pub fn router(self) -> axum::Router {
        Router::new()
            .route("/health", get(|| async { "OK" }))
            .with_state(self)
    }
}

impl AsRef<ProviderBaseService> for ProviderBaseService {
    fn as_ref(&self) -> &ProviderBaseService {
        self
    }
}

#[derive(Clone)]
pub struct ProviderOaiService {
    config: ProviderConfig,
}

impl ProviderOaiService {
    pub fn new(config: ProviderConfig) -> Self {
        info!("Creating ProviderOaiService");
        Self { config }
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
        method: Method,
        host: Host,
        cookies: CookieJar,
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
