use std::convert::Infallible;
use futures::{StreamExt, TryStreamExt};
use tokio::fs;
use warp::{reply, Filter};
use warp::hyper::StatusCode;
use warp::multipart::Part;
use bytes::{BufMut};
use log::info;
use urlencoding::decode;
use uuid::Uuid;
use warp::http::header::CONTENT_TYPE;
use crate::authentication::{validate_auth, Unauthorized};
use crate::models::{Endpoint, Endpoints, RateLimit};
use crate::rate_limit::{check_rate_limit, RateLimitTracker};
use crate::utils::add_possible_delay;

pub async fn register_endpoint(form: warp::multipart::FormData, endpoints: Endpoints) -> Result<impl warp::Reply, warp::Rejection> {
    let mut form = form;
    let mut path = None;
    let mut methods = None;
    let mut status_code = None;
    let mut file_name = None;
    let mut file_data = Vec::new();
    let mut authentication = None;
    let mut delay = None;
    let mut rate_limit = None;

    while let Some(part) = form.next().await {
        let part: Part = part.unwrap();
        let field_name = part.name().to_string();

        if field_name == "path" {
            let value = part_to_string(part).await?;
            path = Some(value);
        } else if field_name == "methods" {
            let value = part_to_string(part).await?;
            methods = Some(value);
        } else if field_name == "status_code" {
            let value = part_to_string(part).await?;
            status_code = Some(value.parse::<u16>().unwrap_or(200));
        } else if field_name == "file" {
            file_name = Some(format!("uploads/{}.json", Uuid::new_v4()));
            file_data = part_to_bytes(part).await?;
        } else if field_name == "authentication" {
            let value = part_to_string(part).await?;
            authentication =  if value == "null" { None } else { Some(value) };
        } else if field_name == "delay" {
            let value = part_to_string(part).await?;
            delay = Some(value.parse::<u64>().ok());
        } else if field_name == "rate_limit" {
            let value = part_to_string(part).await?;

            if !value.is_empty() || value.contains('/') {
                let vals: Vec<&str> = value.split('/').collect();
                rate_limit = Some(RateLimit {
                    requests: vals[0].parse::<usize>().unwrap_or(0),
                    window_ms: vals[1].parse::<u64>().unwrap_or(0),
                });
            } else {
                rate_limit = None;
            }
        }
    }

    let path = path.ok_or_else(|| warp::reject::not_found())?;
    let methods = methods.unwrap().split(",")
        .map(|s| String::from(s)).collect::<Vec<String>>();
    let file_name = file_name.ok_or_else(|| warp::reject::not_found())?;
    let delay = delay.ok_or_else(|| warp::reject::not_found())?;

    fs::write(&file_name, file_data).await.unwrap();

    let endpoint = Endpoint {
        method: methods,
        file: file_name.clone(),
        status_code,
        authentication,
        delay,
        rate_limit,
    };

    endpoints.lock().await.insert(path.clone(), endpoint);

    Ok(reply::json(&"Registered successfully"))
}

async fn part_to_string(part: Part) -> Result<String, warp::Rejection> {
    let bytes = part_to_bytes(part).await?;
    String::from_utf8(bytes)
        .map_err(|_| warp::reject::reject())
}

async fn part_to_bytes(part: Part) -> Result<Vec<u8>, warp::Rejection> {
    let data = part
        .stream()
        .try_fold(Vec::new(), |mut vec, data| {
            vec.put(data);
            async move { Ok(vec) }
        })
        .await
        .map_err(|e| {
            eprintln!("reading file error: {}", e);
            warp::reject::reject()
        })?;

    Ok(data)
}

pub async fn list_endpoint(endpoints: Endpoints) -> Result<impl warp::Reply, warp::Rejection> {
    let endpoints_map = endpoints.lock().await.clone();
    Ok(reply::json(&endpoints_map))
}

pub async fn delete_endpoint(path_to_delete: String, endpoints: Endpoints) -> Result<impl warp::Reply, warp::Rejection> {
    let mut endpoints_map = endpoints.lock().await;
    let decoded_path = decode(&path_to_delete).map_err(|_| warp::reject::not_found())?.into_owned();

    if let Some(endpoint) = endpoints_map.remove(&decoded_path) {
        let file_path = format!("uploads/{}", endpoint.file);

        if fs::remove_file(&file_path).await.is_err() {
            info!("Failed to delete file: {}", file_path);
        }

        return Ok(reply::with_status("Deleted successfully", StatusCode::OK));
    }

    Err(warp::reject::not_found())
}


pub async fn serve_dynamic_response(path: warp::path::FullPath,
                                    auth_header: Option<String>,
                                    endpoints: Endpoints,
                                    rate_limiter: RateLimitTracker
                            ) -> Result<impl warp::Reply, warp::Rejection> {
    let mut endpoint_chosen: Option<Endpoint> = None;

    // Extract what we need from the mutex, then drop the guard
    let endpoint_data = {
        let endpoints_map = endpoints.lock().await;
        if let Some(endpoint) = endpoints_map.get(path.as_str()) {
            endpoint_chosen = Some(endpoint.clone());
            Some((endpoint.file.clone(), endpoint.status_code, endpoint.rate_limit.clone()))
        } else {
            None
        }
    };

    // Now safely await without holding the MutexGuard
    if let Some((file_path, status, rate_limit)) = endpoint_data {
        // First validate authorization if needed
        if let Some(auth) = &endpoint_chosen.clone().unwrap().authentication {
            if !validate_auth(Some(auth.clone()), auth_header) {
                return Err(warp::reject::custom(Unauthorized));
            }
        }

        // Apply rate limiting before processing the request
        check_rate_limit(path.as_str().to_string(), "GET", rate_limit.as_ref(), rate_limiter.clone()).await?;

        //adding possible delay
        if let Some(_) = &endpoint_chosen.clone().unwrap().delay {
            add_possible_delay(&endpoint_chosen.clone().unwrap()).await;
        }

        match tokio::fs::read_to_string(&file_path).await {
            Ok(data) => {
                let status_code = StatusCode::from_u16(status.unwrap_or(200)).unwrap_or(StatusCode::NOT_FOUND);
                let json_response = reply::with_status(data, status_code);
                let json_response = reply::with_header(json_response, CONTENT_TYPE, "application/json");

                Ok(json_response)
            }
            Err(_) => Err(warp::reject::not_found()),
        }
    } else {
        Err(warp::reject::not_found())
    }
}

pub fn with_endpoints(endpoints: Endpoints) -> impl Filter<Extract = (Endpoints,), Error = Infallible> + Clone {
    warp::any().map(move || endpoints.clone())
}