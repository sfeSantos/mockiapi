use warp::{reply, Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::StatusCode;
use crate::middlewares::authentication::{validate_auth};
use crate::models::{Endpoints, NotFound, Unauthorized};
use crate::middlewares::rate_limit::{check_rate_limit, RateLimitTracker};
use crate::utils::add_possible_delay;

pub async fn serve_dynamic_response(
    path: warp::path::FullPath,
    auth_header: Option<String>,
    endpoints: Endpoints,
    rate_limiter: RateLimitTracker,
) -> Result<impl Reply, Rejection> {
    let endpoint_data = {
        let endpoints_map = endpoints.lock().await;
        endpoints_map.get(path.as_str()).cloned()
    };

    if let Some(endpoint) = endpoint_data {
        if let Some(auth) = &endpoint.authentication {
            if !validate_auth(Some(auth.clone()), auth_header) {
                return Err(warp::reject::custom(Unauthorized));
            }
        }

        check_rate_limit(path.as_str().to_string(), "GET", endpoint.rate_limit.as_ref(), rate_limiter.clone()).await?;

        if endpoint.delay.is_some() {
            add_possible_delay(&endpoint).await;
        }

        match tokio::fs::read_to_string(&endpoint.file).await {
            Ok(data) => {
                let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
                    .unwrap_or(StatusCode::NOT_FOUND);
                let response = reply::with_status(data, status_code);
                let response = reply::with_header(response, CONTENT_TYPE, "application/json");

                Ok(response)
            }
            Err(_) => Err(warp::reject::custom(NotFound)),
        }
    } else {
        Err(warp::reject::custom(NotFound))
    }
}