use async_trait::async_trait;
use axum::extract::*;
use axum::routing::get;
use axum::Router;
use axum_extra::extract::CookieJar;
use http::Method;
use tracing::info;

use crate::ProviderConfig;
use openaiapi::apis::completions::{Completions, CreateCompletionResponse};
use openaiapi::apis::models::{
    DeleteModelResponse, ListModelsResponse, Models, RetrieveModelResponse,
};
use openaiapi::models::{
    CreateCompletionRequest, DeleteModelPathParams, Error, RetrieveModelPathParams,
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
        _body: CreateCompletionRequest,
    ) -> Result<CreateCompletionResponse, ()> {
        Ok(CreateCompletionResponse::Status500_InternalServerError(
            Error::new(
                "not_implemented".to_string(),
                "Not implemented".to_string(),
                "None".to_string(),
                "invalid_request_error".to_string(),
            ),
        ))
    }
}
