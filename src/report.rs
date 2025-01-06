use std::time::Duration;

#[derive(Debug)]
pub struct Report {
    pub server_software: String, // server software ( e.g nginx/1.18.0 (Ubuntu) )
    pub host: String, // hostname of the server
    pub port: u16, // port of the server

    pub completed_requests: usize, // total valid request/response cycles
    pub failed_requests: usize, // total number of failed reqeusts
    pub timeouts: usize, // total timeouts
    pub total_html_read: f64, // total html read in KB's
    pub non_2xx_responses: usize, // total non 2.x.x status code responses
    pub concurrency: usize, // concurrency level

    pub duration: Duration, // total duration of the test

    pub latencies: Vec<f64>, // latency of each request in ms (will be used for showing latency distribution)
}

impl Default for Report {
    fn default() -> Self {
        Report {
            server_software: "".to_string(),
            host: "".to_string(),
            port: 0,
            completed_requests: 0,
            failed_requests: 0,
            timeouts: 0,
            total_html_read: 0.0,
            non_2xx_responses: 0,
            concurrency: 0,

            duration: Duration::from_secs(0),
            latencies: Vec::new(),
        }
    }
}
