use std::env;
use std::{ slice::Iter, iter::Skip };
use std::time::Duration;
use url::Url;

// Error messages
const ERR_INVALID_REQUESTS: &str = "Invalid number of requests\nUse --help for more info";
const ERR_INVALID_CONCURRENCY: &str =
    "Invalid number of concurrent requests\nUse --help for more info";
const ERR_INVALID_THREADS: &str = "Invalid number of threads\nUse --help for more info";
const ERR_INVALID_DURATION: &str = "Invalid value for duration\nUse --help for more info";
const ERR_INVALID_TIMEOUT: &str = "Invalid value for timeout\nUse --help for more info";
const ERR_INVALID_CONNECTION_TIMEOUT: &str =
    "Invalid value for connection-timeout\nUse --help for more info";
const ERR_URL_NOT_PROVIDED: &str = "URL not provided\nUse --help for more info";
const ERR_INVALID_URL: &str = "Invalid URL\nUse --help for more info";
const ERR_INVALID_REQUESTS_AND_CONCURRENCY: &str =
    "Number of requests must be >= concurrency\nUse --help for more info";

// Type of test to run
#[derive(Debug, PartialEq, Clone)]
pub enum TestType {
    RequestCount,
    Duration,
    Both,
}

// Parse arguments for CLI
#[derive(Debug, Clone)]
pub struct Config {
    pub requests: usize,
    pub duration: Duration,
    pub test_type: TestType,

    pub concurrency: usize, // number of concurrent requests
    pub threads: usize,
    pub timeout: Duration, // total time for request/response cycle including DNS resolution
    pub connection_timeout: Duration, // timeout for establishing connection to the host (not the complete request/response cycle)
    pub summarize: bool, // summarize the output

    pub url: String,
}

// Default values, uses total requests test by default
impl Default for Config {
    fn default() -> Self {
        Config {
            requests: 10,
            duration: Duration::from_secs(0),
            test_type: TestType::RequestCount,
            concurrency: 1,
            threads: 1,
            timeout: Duration::from_secs(25),
            connection_timeout: Duration::from_secs(20),
            summarize: false,
            url: "".to_string(),
        }
    }
}

impl Config {
    /*-------------------- Public Functions -------------------*/
    pub fn parse() -> Config {
        let mut parsed_config = Self::default();
        let args: Vec<String> = env::args().collect();

        if args.len() == 1 {
            // no arguments given
            Self::print_help();
            std::process::exit(0);
        }

        let mut args_iter = args.iter().skip(1); // skip the first argument
        let mut url_provided = false; // so the url is not taken more than once
        let mut req_count_test_provided: bool = false; // for setting TestType as Both

        while let Some(arg) = args_iter.next() {
            if Self::handle_help(arg) || Self::handle_version(arg) {
                // check for -h / --help  and -v / --version flags
                std::process::exit(0);
            }

            if
                Self::handle_duration_test(
                    &mut parsed_config,
                    arg,
                    &mut args_iter,
                    &mut req_count_test_provided
                ) ||
                Self::handle_request_count_test(
                    &mut parsed_config,
                    arg,
                    &mut args_iter,
                    &mut req_count_test_provided
                ) ||
                Self::handle_concurrency(&mut parsed_config, arg, &mut args_iter) ||
                Self::handle_threads(&mut parsed_config, arg, &mut args_iter) ||
                Self::handle_timeout(&mut parsed_config, arg, &mut args_iter) ||
                Self::handle_connection_timeout(&mut parsed_config, arg, &mut args_iter) ||
                Self::handle_summarize(&mut parsed_config, arg) ||
                Self::handle_url(&mut parsed_config, arg, &mut url_provided)
            {
                continue;
            } else {
                Self::print_help();
                std::process::exit(1);
            }
        }

        if !url_provided {
            eprintln!("{}", ERR_URL_NOT_PROVIDED);
            std::process::exit(1);
        }

        if parsed_config.concurrency > parsed_config.requests {
            eprintln!("{}", ERR_INVALID_REQUESTS_AND_CONCURRENCY);
            std::process::exit(1);
        }

        parsed_config
    }

    pub fn print_help() {
        let name = env!("CARGO_PKG_NAME");
        println!("Usage: {} [OPTIONS] <URL>", name);
        println!();
        println!("{} powered by nayaraasta", name);
        println!();
        println!("Options:");
        println!("  -n, --requests           <N>  Number of requests (Default: 10)");
        println!("  -d, --duration           <D>  Test duration");
        println!("  -c, --concurrency        <N>  Concurrent requests (Default: 1)");
        println!("  -t, --threads            <N>  Number of threads (Default: 1)");
        println!("  -T, --timeout            <D>  Request timeout (Default: 25s)");
        println!("  -C, --connection-timeout <D>  Connection timeout (Default: 20s)");
        println!("  -s                            Summarize output");
        println!("  -h, --help                    Print help (this)");
        println!("  -v, --version                 Print version");
        println!();
        println!("Arguments:");
        println!("  <URL>                         URL to test");
        println!();
        println!("Durations can be specified like: 10s, 1m, 1h");
        println!("The test ends when either -n or -d completes. (if both are given)");
    }

    /*---------------- Private/Helpers ------------------*/
    fn handle_duration_test(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>,
        req_count_test_provided: &mut bool
    ) -> bool {
        if arg.starts_with("-d") || arg.starts_with("--duration") {
            Self::parse_duration(parsed_config, arg, args_iter);
            if *req_count_test_provided {
                parsed_config.test_type = TestType::Both;
            } else {
                parsed_config.test_type = TestType::Duration;
            }
            true
        } else {
            false
        }
    }

    fn handle_request_count_test(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>,
        req_count_test_provided: &mut bool
    ) -> bool {
        if arg.starts_with("-n") || arg.starts_with("--requests") {
            Self::parse_requests(parsed_config, arg, args_iter);
            if parsed_config.test_type == TestType::Duration {
                parsed_config.test_type = TestType::Both;
            }
            *req_count_test_provided = true;
            true
        } else {
            false
        }
    }

    fn handle_concurrency(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>
    ) -> bool {
        if arg.starts_with("-c") || arg.starts_with("--concurrency") {
            Self::parse_concurrency(parsed_config, arg, args_iter);
            true
        } else {
            false
        }
    }

    fn handle_threads(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>
    ) -> bool {
        if arg.starts_with("-t") || arg.starts_with("--threads") {
            Self::parse_threads(parsed_config, arg, args_iter);
            true
        } else {
            false
        }
    }

    fn handle_timeout(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>
    ) -> bool {
        if arg.starts_with("-T") || arg.starts_with("--timeout") {
            Self::parse_timeout(parsed_config, arg, args_iter);
            true
        } else {
            false
        }
    }

    fn handle_connection_timeout(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>
    ) -> bool {
        if arg.starts_with("-C") || arg.starts_with("--connection-timeout") {
            Self::parse_connection_timeout(parsed_config, arg, args_iter);
            true
        } else {
            false
        }
    }

    fn handle_summarize(parsed_config: &mut Config, arg: &str) -> bool {
        if arg == "-s" {
            parsed_config.summarize = true;
            true
        } else {
            false
        }
    }

    fn handle_help(arg: &str) -> bool {
        if arg == "-h" || arg == "--help" {
            Self::print_help();
            true
        } else {
            false
        }
    }

    fn handle_version(arg: &str) -> bool {
        if arg == "-v" || arg == "--version" {
            let name = env!("CARGO_PKG_NAME");
            let version = env!("CARGO_PKG_VERSION");
            println!("{} {}", name, version);
            true
        } else {
            false
        }
    }
    fn handle_url(parsed_config: &mut Config, arg: &str, is_url_set: &mut bool) -> bool {
        if !*is_url_set && !arg.starts_with("-") {
            Self::parse_url(parsed_config, arg);
            *is_url_set = true;
            true
        } else {
            false
        }
    }

    fn parse_url(parsed_config: &mut Config, url: &str) {
        // Check if the url is correct
        if !Url::parse(url).is_ok() {
            println!("\"{}\"\n{}", url, ERR_INVALID_URL);
            std::process::exit(1);
        }
        parsed_config.url = url.to_string();
    }

    fn parse_requests(parsed_config: &mut Config, arg: &str, args_iter: &mut Skip<Iter<String>>) {
        if let Some(strip) = arg.strip_prefix("-n") {
            parsed_config.requests = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_usize(args_iter, strip, &ERR_INVALID_REQUESTS)
                );
        } else if let Some(strip) = arg.strip_prefix("--requests") {
            parsed_config.requests = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_usize(args_iter, strip, &ERR_INVALID_REQUESTS)
                );
        } else {
            eprintln!("{}", ERR_INVALID_REQUESTS);
            std::process::exit(1);
        }

        if parsed_config.requests == 0 {
            eprintln!("{}", ERR_INVALID_REQUESTS);
            std::process::exit(1);
        }
    }

    fn parse_concurrency(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>
    ) {
        if let Some(strip) = arg.strip_prefix("-c") {
            parsed_config.concurrency = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_usize(args_iter, strip, &ERR_INVALID_CONCURRENCY)
                );
        } else if let Some(strip) = arg.strip_prefix("--concurrency") {
            parsed_config.concurrency = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_usize(args_iter, strip, &ERR_INVALID_CONCURRENCY)
                );
        } else {
            eprintln!("{}", ERR_INVALID_CONCURRENCY);
            std::process::exit(1);
        }

        if parsed_config.concurrency == 0 {
            eprintln!("{}", ERR_INVALID_CONCURRENCY);
            std::process::exit(1);
        }
    }

    fn parse_threads(parsed_config: &mut Config, arg: &str, args_iter: &mut Skip<Iter<String>>) {
        if let Some(strip) = arg.strip_prefix("-t") {
            parsed_config.threads = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_usize(args_iter, strip, &ERR_INVALID_THREADS)
                );
        } else if let Some(strip) = arg.strip_prefix("--threads") {
            parsed_config.threads = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_usize(args_iter, strip, &ERR_INVALID_THREADS)
                );
        } else {
            eprintln!("{}", ERR_INVALID_THREADS);
            std::process::exit(1);
        }

        if parsed_config.threads == 0 {
            eprintln!("{}", ERR_INVALID_THREADS);
            std::process::exit(1);
        }
    }

    // for -n 10 (space between flag and value)
    fn parse_with_next_usize(
        args_iter: &mut Skip<Iter<String>>,
        strip: &str,
        error_msg: &str
    ) -> usize {
        if !strip.is_empty() {
            eprintln!("{}", error_msg); // other (invalid) characters were written after the flag
            std::process::exit(1);
        }
        args_iter
            .next()
            .and_then(|next| next.parse().ok())
            .unwrap_or_else(|| {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            })
    }

    /* ----Durations ----*/
    // Parse the duration flag
    fn parse_duration(parsed_config: &mut Config, arg: &str, args_iter: &mut Skip<Iter<String>>) {
        let duration_str: String;
        if let Some(strip) = arg.strip_prefix("-d") {
            duration_str = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_duration(args_iter, &ERR_INVALID_DURATION)
                );
        } else if let Some(strip) = arg.strip_prefix("--duration") {
            duration_str = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_duration(args_iter, &ERR_INVALID_DURATION)
                );
        } else {
            eprintln!("{}", ERR_INVALID_DURATION);
            std::process::exit(1);
        }
        parsed_config.duration = Self::parse_duration_string(&duration_str, &ERR_INVALID_DURATION);
        if parsed_config.duration.as_secs() == 0 {
            eprintln!("{}", ERR_INVALID_DURATION);
            std::process::exit(1);
        }
    }

    fn parse_timeout(parsed_config: &mut Config, arg: &str, args_iter: &mut Skip<Iter<String>>) {
        let duration_str: String;
        if let Some(strip) = arg.strip_prefix("-T") {
            duration_str = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_duration(args_iter, &ERR_INVALID_TIMEOUT)
                );
        } else if let Some(strip) = arg.strip_prefix("--timeout") {
            duration_str = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_duration(args_iter, &ERR_INVALID_TIMEOUT)
                );
        } else {
            eprintln!("{}", ERR_INVALID_TIMEOUT);
            std::process::exit(1);
        }
        parsed_config.timeout = Self::parse_duration_string(&duration_str, &ERR_INVALID_TIMEOUT);
        if parsed_config.timeout.as_secs() == 0 {
            eprintln!("{}", ERR_INVALID_TIMEOUT);
            std::process::exit(1);
        }
    }

    fn parse_connection_timeout(
        parsed_config: &mut Config,
        arg: &str,
        args_iter: &mut Skip<Iter<String>>
    ) {
        let duration_str: String;
        if let Some(strip) = arg.strip_prefix("-C") {
            duration_str = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_duration(args_iter, &ERR_INVALID_CONNECTION_TIMEOUT)
                );
        } else if let Some(strip) = arg.strip_prefix("--connection-timeout") {
            duration_str = strip
                .parse()
                .unwrap_or_else(|_|
                    Self::parse_with_next_duration(args_iter, &ERR_INVALID_CONNECTION_TIMEOUT)
                );
        } else {
            eprintln!("{}", ERR_INVALID_CONNECTION_TIMEOUT);
            std::process::exit(1);
        }
        parsed_config.connection_timeout = Self::parse_duration_string(
            &duration_str,
            &ERR_INVALID_CONNECTION_TIMEOUT
        );
        if parsed_config.connection_timeout.as_secs() == 0 {
            eprintln!("{}", ERR_INVALID_CONNECTION_TIMEOUT);
            std::process::exit(1);
        }
    }

    // Parse the duration value
    fn parse_with_next_duration(args_iter: &mut Skip<Iter<String>>, error_msg: &str) -> String {
        args_iter
            .next()
            .and_then(|next| next.parse().ok())
            .unwrap_or_else(|| {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            })
    }

    // Parses the duration string and returns Duration struct
    fn parse_duration_string(duration_str: &str, error_msg: &str) -> Duration {
        // nothing specified after -d or --duration
        if duration_str.len() == 0 {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        }

        // if no unit is provided use seconds "s"
        let duration_str: String = if
            duration_str.ends_with("s") ||
            duration_str.ends_with("m") ||
            duration_str.ends_with("h")
        {
            duration_str.to_string()
        } else {
            format!("{}s", duration_str)
        };

        // split into value and unit for (s, m, h)
        let (value_str, unit) = duration_str.split_at(duration_str.len() - 1);
        let value: u64 = value_str.parse().unwrap_or_else(|_| {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        });

        match unit {
            "s" => Duration::from_secs(value),
            "m" => Duration::from_secs(value * 60),
            "h" => Duration::from_secs(value * 60 * 60),
            _ => Duration::from_secs(value),
        }
    }
}
