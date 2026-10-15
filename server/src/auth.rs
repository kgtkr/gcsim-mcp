use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};

pub async fn auth_middleware(
    expected_token: Option<String>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ref token) = expected_token {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        match auth_header {
            Some(header_val) if header_val.starts_with("Bearer ") => {
                let provided = &header_val["Bearer ".len()..];
                if provided.trim() != token {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }

    Ok(next.run(req).await)
}
