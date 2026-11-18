use std::sync::Arc;
use bytes::BufMut;
use futures::{StreamExt, TryStreamExt};
use tokio::fs;
use uuid::Uuid;
use warp::{Rejection};
use warp::multipart::Part;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{Endpoint, EndpointKind, Endpoints, FileError, InvalidMultipart, InvalidRegistration, MultipartHandler, NotFound, RateLimit, Utf8Error};
use crate::models::grpc::GrpcMockResponse;

/// Directory the uploaded mock files are written to, relative to the working directory.
pub const UPLOADS_DIR: &str = "uploads";

impl MultipartHandler {
    
    pub async fn parse(form: warp::multipart::FormData, endpoints: Endpoints, grpc_registry: Arc<GrpcRegistry>) -> Result<(), Rejection> {
        let mut path = None;
        let mut methods = None;
        let mut status_code = None;
        let mut file_name = None;
        let mut file_data = Vec::new();
        let mut authentication = None;
        let mut delay = None;
        let mut rate_limit = None;
        let mut with_dynamic_vars = None;
        let mut parts = form.into_stream();
        let mut grpc_service = None;
        let mut grpc_method = None;
        let mut is_graphql = false;
        
        while let Some(Ok(part)) = parts.next().await {
            match part.name() {
                "path" => path = Some(Self::part_to_string(part).await?),
                "methods" => methods = Some(Self::part_to_string(part).await?),
                "status_code" => status_code = Some(Self::part_to_string(part).await?.parse::<u16>().unwrap_or(200)),
                "file" => {
                    file_name = Some(format!("{UPLOADS_DIR}/{}.json", Uuid::new_v4()));
                    file_data = Self::part_to_bytes(part).await?;
                }
                "authentication" => {
                    let value = Self::part_to_string(part).await?;
                    authentication = if value == "null" { None } else { Some(value) };
                }
                "delay" => delay = Some(Self::part_to_string(part).await?.parse::<u64>().ok()),
                "rate_limit" => rate_limit = Self::parse_rate_limit(&Self::part_to_string(part).await?)?,
                "with_dynamic_vars" => {
                    with_dynamic_vars = Some(Self::part_to_string(part).await?
                        .parse::<bool>().unwrap_or(false));
                },
                "isGraphQL" => is_graphql = Self::part_to_string(part).await? == "true",
                "grpcService" => {
                    // The form always sends this field, empty for REST and GraphQL.
                    let value = Self::part_to_string(part).await?;
                    grpc_service = (!value.is_empty()).then_some(value);
                }
                "grpcRPC" => {
                    let value = Self::part_to_string(part).await?;
                    grpc_method = (!value.is_empty()).then_some(value);
                }
                _ => {}
            }
        }

        let path = path.ok_or_else(|| warp::reject::custom(NotFound))?;
        let methods = methods
            .map(|m| m.split(',').map(String::from).collect())
            .unwrap_or_else(Vec::new);
        let file_name = file_name.ok_or_else(|| warp::reject::custom(NotFound))?;
        let delay = delay.unwrap_or(None);

        if grpc_service.is_some() && grpc_method.is_none() {
            return Err(warp::reject::custom(InvalidRegistration(
                "grpcRPC is required when grpcService is set",
            )));
        }

        fs::write(&file_name, file_data.clone())
            .await
            .map_err(|_| warp::reject::custom(FileError))?;
        
        let kind = if grpc_service.is_some() {
            EndpointKind::Grpc
        } else if is_graphql {
            EndpointKind::GraphQl
        } else {
            EndpointKind::Rest
        };

        let endpoint = Endpoint {
            kind,
            method: methods,
            file: file_name,
            status_code,
            authentication,
            delay,
            rate_limit,
            with_dynamic_vars,
        };
        
        if let (Some(grpc), Some(rpc)) = (grpc_service, grpc_method) {
            let data = serde_json::from_slice(&file_data)
                .map_err(|_| warp::reject::custom(Utf8Error))?;
            grpc_registry.register_mock(&grpc, &rpc, GrpcMockResponse{
                output: data,
                delay_ms: delay,
                status: status_code,
            }).await
        }

        let replaced = endpoints.lock().await.insert(path, endpoint);

        // Registering a path again supersedes its previous upload.
        if let Some(replaced) = replaced {
            if fs::remove_file(&replaced.endpoint.file).await.is_err() {
                log::info!("Failed to delete file: {}", replaced.endpoint.file);
            }
        }
        
        Ok(())
    }

    /// Reads `<requests>/<window in ms>`. An empty value means no rate limit.
    fn parse_rate_limit(value: &str) -> Result<Option<RateLimit>, Rejection> {
        if value.is_empty() {
            return Ok(None);
        }

        let invalid = || {
            warp::reject::custom(InvalidRegistration(
                "rate_limit must be <requests>/<window in ms>, for example 10/1000",
            ))
        };
        let (requests, window_ms) = value.split_once('/').ok_or_else(invalid)?;
        let requests = requests.trim().parse::<usize>().map_err(|_| invalid())?;
        let window_ms = window_ms.trim().parse::<u64>().map_err(|_| invalid())?;

        Ok(Some(RateLimit { requests, window_ms }))
    }

    async fn part_to_string(part: Part) -> Result<String, Rejection> {
        let bytes = Self::part_to_bytes(part).await?;
        String::from_utf8(bytes).map_err(|_| warp::reject::custom(Utf8Error))
    }

    async fn part_to_bytes(part: Part) -> Result<Vec<u8>, Rejection> {
        part.stream()
            .try_fold(Vec::new(), |mut vec, data| async move {
                vec.put(data);
                Ok(vec)
            })
            .await
            .map_err(|_| warp::reject::custom(InvalidMultipart))
    }
}