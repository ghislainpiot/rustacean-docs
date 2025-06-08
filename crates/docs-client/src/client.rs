use reqwest::Client;

#[allow(dead_code)]
pub struct DocsClient {
    client: Client,
}

impl DocsClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for DocsClient {
    fn default() -> Self {
        Self::new()
    }
}
