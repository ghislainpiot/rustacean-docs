use reqwest::Client;

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