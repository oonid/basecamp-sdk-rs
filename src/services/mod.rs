mod authorization;

pub use authorization::{Authorization, AuthorizationService};

pub struct ProjectsService {
    http: std::sync::Arc<crate::http::HttpClient>,
}

impl ProjectsService {
    pub fn new(http: std::sync::Arc<crate::http::HttpClient>) -> Self {
        Self { http }
    }

    pub fn http(&self) -> &crate::http::HttpClient {
        &self.http
    }
}
