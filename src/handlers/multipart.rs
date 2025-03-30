use bytes::BufMut;
use futures::{StreamExt, TryStreamExt};
use tokio::fs;
use uuid::Uuid;
use warp::{Rejection};
use warp::multipart::Part;
use crate::models::{Endpoint, Endpoints, FileError, InvalidMultipart, MultipartHandler, NotFound, RateLimit, Utf8Error};

impl MultipartHandler {
    
    pub async fn parse(form: warp::multipart::FormData, endpoints: Endpoints) -> Result<(), Rejection> {
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
        
        while let Some(Ok(part)) = parts.next().await {
            match part.name() {
                "path" => path = Some(Self::part_to_string(part).await?),
                "methods" => methods = Some(Self::part_to_string(part).await?),
                "status_code" => status_code = Some(Self::part_to_string(part).await?.parse::<u16>().unwrap_or(200)),
                "file" => {
                    file_name = Some(format!("uploads/{}.json", Uuid::new_v4()));
                    file_data = Self::part_to_bytes(part).await?;
                }
                "authentication" => {
                    let value = Self::part_to_string(part).await?;
                    authentication = if value == "null" { None } else { Some(value) };
                }
                "delay" => delay = Some(Self::part_to_string(part).await?.parse::<u64>().ok()),
                "rate_limit" => {
                    let value = Self::part_to_string(part).await?;
                    if !value.is_empty() || value.contains('/') {
                        let vals: Vec<&str> = value.split('/').collect();
                        rate_limit = Some(RateLimit {
                            requests: vals[0].parse::<usize>().unwrap_or(0),
                            window_ms: vals[1].parse::<u64>().unwrap_or(0),
                        });
                    }
                },
                "with_dynamic_vars" => {
                    with_dynamic_vars = Some(Self::part_to_string(part).await?
                        .parse::<bool>().unwrap_or(false));
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

        fs::write(&file_name, file_data)
            .await
            .map_err(|_| warp::reject::custom(FileError))?;
        
        let endpoint = Endpoint {
            method: methods,
            file: file_name,
            status_code,
            authentication,
            delay,
            rate_limit,
            with_dynamic_vars,
        };

        endpoints.lock().await.insert(path.clone(), endpoint);
        
        Ok(())
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