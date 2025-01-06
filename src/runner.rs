use crate::config::{ Config, TestType };
use crate::report::Report;

use std::sync::{ Arc, Mutex, atomic::{ AtomicBool, Ordering } };
use std::net::TcpStream;
use std::io::Write;
use tokio::time::Instant;
use url::Url;
use isahc::{
    HttpClient,
    HttpClientBuilder,
    config::Configurable,
    error::ErrorKind,
    AsyncReadResponseExt,
};
use tokio::{ sync::Notify, runtime::{ Builder, Runtime }, time::{ sleep, Duration } };
// use ctrlc;

const FIELD_WIDTH: usize = 24; //  width of each field for formatting print
const BUCKET_COUNT: usize = 10; // size of the histogram

/// Runner structure with configuration and a shared report.
#[derive(Debug, Clone)]
pub struct Runner {
    config: Config,
    report: Arc<Mutex<Report>>, // final report
    client: HttpClient, // client for sending requests
}

impl Runner {
    /*------------------==| Public Functions |==-------------------------*/
    /// Create a new Runner instance
    pub fn new(config: Config) -> Self {
        let client = HttpClientBuilder::new()
            .timeout(config.timeout)
            .connect_timeout(config.connection_timeout)
            .build()
            .unwrap();

        let mut report = Report::default();
        report.concurrency = config.concurrency; // set the concurrency in report

        Self {
            config,
            report: Arc::new(Mutex::new(report)),
            client,
        }
    }

    /// Main entry point to run the benchmarking tool
    pub fn run(self) -> Result<Self, String> {
        // check if the url is reachable
        if let Err(_) = self.is_url_reachable(&self.config.url) {
            return Err(format!("Failed to resolve {}", self.config.url));
        }
        if self.config.test_type == TestType::RequestCount {
            Ok(self.run_req_count_test())
        } else if self.config.test_type == TestType::Duration {
            Ok(self.run_duration_test())
        } else {
            Ok(self.run_both_tests())
        }
    }

    /// Print the benchmarking report
    pub fn print_report(&self) {
        if self.config.summarize {
            self.print_summarized_report();
        } else {
            self.print_full_report();
        }
    }

    /*-------------------==| Private/Helpers |==----------------------- */

    /// Run the RequestCount test
    fn run_req_count_test(self) -> Self {
        let runtime = Self::get_arc_runtime(&self.config.threads);

        // To share runner across different async tasks
        let runner = Arc::new(self);

        // to stop the timer thread when all of the requests are finished
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawns a threads that stops the test after given duration
        Self::spawn_timer_thread(Arc::clone(&runner), stop_flag.clone());

        // new tokio async runtime
        runtime.block_on(async {
            // Run total batches
            let total_batches = runner.config.requests / runner.config.concurrency;
            for batch in 1..=total_batches {
                let _ = Self::run_batch(runner.clone(), runner.config.concurrency).await;
                print!("\rCompleted requests: {}", batch * runner.config.concurrency); // move to the start of line and print
                std::io::stdout().flush().unwrap(); // ensure the output is displayed immediately
            }

            // Run remainder
            let remainder = runner.config.requests % runner.config.concurrency;
            if remainder > 0 {
                let _ = Self::run_batch(runner.clone(), remainder).await;
                print!("\rCompleted requests: {}", runner.config.requests); // move to the start of line and print
                std::io::stdout().flush().unwrap(); // ensure the output is displayed immediately
            }
            stop_flag.store(true, Ordering::Relaxed);
            sleep(Duration::from_millis(10)).await; // wait for the timer_thread to stop
        });

        Arc::try_unwrap(runner).unwrap_or_else(|_|
            panic!("Runner instance still has active references.")
        )
    }

    /// Run duration test
    fn run_duration_test(self) -> Self {
        // new tokio async runtime
        let runtime = Self::get_arc_runtime(&self.config.threads);

        // To share runner across different async tasks
        let runner = Arc::new(self);

        // notify signal to stop the loop
        let notify = Arc::new(Notify::new());

        // Spawns a threads that stops the test after given duration by notifying
        Self::spawn_duration_thread(Arc::clone(&runner), notify.clone());

        runtime.block_on(async {
            // Infinite loop to keep sending requests till time ends
            loop {
                tokio::select! {
                    _ = Self::run_batch(runner.clone(), runner.config.concurrency)=>{}
                    _ = notify.notified() => { break; } // break the loop on notify signal
                }
            }
        });

        // drop the runtime to release any references to runner
        drop(runtime);

        Arc::try_unwrap(runner).unwrap_or_else(|_|
            panic!("Runner instance still has active references.")
        )
    }

    /// Run both tests whichever one finishes first will stop the test
    fn run_both_tests(self) -> Self {
        // new tokio async runtime
        let runtime = Self::get_arc_runtime(&self.config.threads);

        // To share runner across different async tasks
        let runner = Arc::new(self);

        // Stop flag to clear references of runner if total requests finish before duration
        let stop_flag = Arc::new(AtomicBool::new(false));

        // notify signal to stop the loop
        let notify = Arc::new(Notify::new());

        // Spawns a threads that stops the test after given duration
        Self::spawn_duration_thread_with_flag(
            Arc::clone(&runner),
            notify.clone(),
            stop_flag.clone()
        );

        runtime.block_on(async {
            // Run total batches
            let total_batches = runner.config.requests / runner.config.concurrency;
            for _ in 0..total_batches {
                tokio::select! {
                     _ = Self::run_batch(runner.clone(), runner.config.concurrency) =>{}
                     _ = notify.notified() => { break; }
                }
            }

            // Run remainder
            let remainder = runner.config.requests % runner.config.concurrency;
            if remainder > 0 {
                tokio::select! {
                     _ = Self::run_batch(runner.clone(), remainder) => {}
                     _ = notify.notified() => { return; }
                }
            }
            stop_flag.store(true, Ordering::Relaxed);
            sleep(Duration::from_millis(10)).await; // wait for the duration_thread to stop
        });

        // drop the runtime to release runner references (if any)
        drop(runtime);

        Arc::try_unwrap(runner).unwrap_or_else(|_|
            panic!("Runner instance still has active references.")
        )
    }

    /// Helper function for running batches
    async fn run_batch(
        runner: Arc<Runner>,
        count: usize
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut handles = Vec::new();
        for _ in 0..count {
            let runner = runner.clone();
            handles.push(tokio::spawn(async move { runner.send_request(&runner.client).await }));
        }
        for handle in handles {
            handle.await??;
        }
        Ok(())
    }

    /// Send the request
    async fn send_request(&self, client: &HttpClient) -> Result<(), isahc::Error> {
        let start = Instant::now();

        let response = client.get_async(self.config.url.clone()).await;

        let latency = start.elapsed();

        match response {
            Ok(mut res) => {
                let html_read = res.text().await?.len();
                let mut report = self.report.lock().unwrap();

                report.total_html_read += (html_read as f64) / 1024.0; // in KB's
                report.latencies.push(latency.as_millis() as f64); // push latency for current request
                report.completed_requests += 1; // increment completed requests

                // non 2.x.x responses
                if res.status().as_u16() / 100 != 2 {
                    report.non_2xx_responses += 1;
                }

                // Set the server software
                if report.server_software.is_empty() {
                    if let Some(server_header) = res.headers().get("server") {
                        report.server_software = server_header
                            .to_str()
                            .unwrap_or_default()
                            .to_string();
                    }
                }
            }
            Err(err) => {
                let mut report = self.report.lock().unwrap();
                report.failed_requests += 1; // increment number of failed requests
                if err.kind() == ErrorKind::Timeout {
                    // timeout was reached
                    report.timeouts += 1;
                }
            }
        }
        Ok(())
    }

    // std::Thread to stop the test after given duration (also prints and updates the elapsed time)
    fn spawn_duration_thread(runner: Arc<Runner>, notify: Arc<Notify>) {
        std::thread::spawn(move || {
            let duration = runner.config.duration;
            let start = Instant::now();
            let mut last_printed_second = 0; // keep track of the last printed second

            while start.elapsed() <= duration {
                // for printing progress
                let elapsed = start.elapsed().as_secs(); // get elapsed time in seconds
                if start.elapsed() < duration && elapsed > last_printed_second {
                    last_printed_second = elapsed;
                    print!("\rElapsed time: {}s", elapsed); // move to the start of line and print
                    std::io::stdout().flush().unwrap(); // ensure the output is displayed immediately
                }
                runner.report.lock().unwrap().duration = start.elapsed(); // keep updating the test duration for ctrlc
                std::thread::sleep(Duration::from_millis(10)); // delay to keep printing the progress
            }
            notify.notify_waiters();
        });
    }

    /* ---------------------------------------------------------------------------
     * std::Thread for running both tests, as soon as the total requests finishes
     * a flag is set which terminates this thread, if above function is used
     * an error will be generated on Arc::try_unwrap(runner)
     * because of active references
     * (also prints and updates the elapsed time)
     * ------------------------------------------------------------------------ */
    fn spawn_duration_thread_with_flag(
        runner: Arc<Runner>,
        notify: Arc<Notify>,
        stop_flag: Arc<AtomicBool>
    ) {
        std::thread::spawn(move || {
            let duration = runner.config.duration;
            let start = Instant::now();
            let mut last_printed_second = 0; // keep track of the last printed second

            while start.elapsed() <= duration {
                if stop_flag.load(Ordering::Relaxed) {
                    return; // return immediately if the flag is set
                }

                // for printing progress
                let elapsed = start.elapsed().as_secs(); // get elapsed time in seconds
                if start.elapsed() < duration && elapsed > last_printed_second {
                    last_printed_second = elapsed;
                    print!("\rElapsed time: {}s", elapsed); // move to the start of line and print
                    std::io::stdout().flush().unwrap(); // ensure the output is displayed immediately
                }

                runner.report.lock().unwrap().duration = start.elapsed(); // keep updating the test duration for ctrlc
                std::thread::sleep(Duration::from_millis(10)); // small delay to keep checking for flag
            }
            // Otherwise notify waiters
            notify.notify_waiters();
        });
    }

    /// To update the elapsed time in request_count test
    fn spawn_timer_thread(runner: Arc<Runner>, stop_flag: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let start = Instant::now();

            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    return; // return immediately if the flag is set
                }

                runner.report.lock().unwrap().duration = start.elapsed(); // keep updating the test duration for ctrlc
                std::thread::sleep(Duration::from_millis(10)); // delay to keep printing the progress
            }
        });
    }

    /// Helper function to create tokio Arc runtime
    fn get_arc_runtime(threads: &usize) -> Arc<Runtime> {
        Arc::new(
            Builder::new_multi_thread()
                .worker_threads(*threads)
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime")
        )
    }

    /// Check if the URL is reachable before running tests
    fn is_url_reachable(&self, url: &str) -> Result<(), Box<dyn std::error::Error>> {
        let parsed_url = Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;
        let hostname = parsed_url
            .host_str()
            .ok_or_else(|| "URL does not have a valid hostname".to_string())?;
        let port = parsed_url.port_or_known_default().unwrap_or(80); // HTTP port 80 if none sepcified

        // set the hostname and port in report
        let mut report = self.report.lock().unwrap();
        report.host = hostname.to_string();
        report.port = port;

        let address = format!("{}:{}", hostname, port);
        match TcpStream::connect(address) {
            Ok(_) => {
                if self.config.test_type == TestType::RequestCount {
                    println!("Sending {} request(s) to {}", self.config.requests, self.config.url);
                } else if self.config.test_type == TestType::Duration {
                    println!(
                        "Running {}s test on {}",
                        self.config.duration.as_secs(),
                        self.config.url
                    );
                } else {
                    println!(
                        "Sending {} request(s) to {} in {}s",
                        self.config.requests,
                        self.config.url,
                        self.config.duration.as_secs()
                    );
                }
                println!(
                    "using {} thread(s) and {} connection(s)\nPlease be patient..",
                    self.config.threads,
                    self.config.concurrency
                );
                Ok(())
            }
            Err(e) => Err(format!("Failed to connect: {}", e).into()),
        }
    }

    /*---------= Everything related to printing =----------*/
    fn print_summarized_report(&self) {
        print!("\n\n");

        let mut report = self.report.lock().unwrap();

        Self::print_report_details_summary(&report);

        // convert latencies in ms
        report.latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        Self::print_request_timings_summary(&report.latencies);
        Self::print_latency_distribution(&report.latencies);
        Self::print_report_throughput_summary(&report);
    }

    /// Prints Throughput like req/sec and data_transfer/sec
    fn print_report_throughput_summary(report: &Report) {
        let duration = report.duration.as_secs_f64();
        println!(
            "{:<20} {:>7.3}",
            "Request(s) per sec:",
            (report.completed_requests as f64) / duration
        );
        println!(
            "{:<20} {:>7.3} KB (html)",
            "Transfer per sec:",
            (report.total_html_read as f64) / duration
        );
    }

    fn print_report_details_summary(report: &Report) {
        println!(
            "Sent {} requests in {:.2}s, {:.3}KB read (html)",
            report.completed_requests,
            report.duration.as_secs_f64(),
            report.total_html_read
        );
        if report.failed_requests > 0 {
            println!(
                "Failed Requests: {}, out of which timeouts {}",
                report.failed_requests,
                report.timeouts
            );
        }
    }

    /// Print request timings for summarized report
    fn print_request_timings_summary(latencies_ms: &Vec<f64>) {
        let mean = latencies_ms.iter().sum::<f64>() / (latencies_ms.len() as f64); // calculate mean
        let variance: f64 = // calculate variance
            latencies_ms
                .iter()
                .map(|&value| (value - mean).powi(2))
                .sum::<f64>() / (latencies_ms.len() as f64);
        let stdev = variance.sqrt(); // calculate standard deviation
        let min = latencies_ms.iter().cloned().fold(f64::INFINITY, f64::min); // minimum latency
        let max = latencies_ms.iter().cloned().fold(f64::NEG_INFINITY, f64::max); // maximum latency

        println!("Latnecy Stats:");
        println!(" {:<10} {:<10} {:<10} {:<10}", "Avg", "Min", "Max", "Stdev");
        println!(
            " {:<10} {:<10} {:<10} {:<10}",
            Self::format_latency(mean),
            Self::format_latency(min),
            Self::format_latency(max),
            Self::format_latency(stdev)
        );
    }

    // convert into seconds if the value is greater than 1000ms
    fn format_latency(value: f64) -> String {
        if value > 1000.0 {
            format!("{:.2}s", value / 1000.0) // convert to seconds
        } else {
            format!("{:.2}ms", value) // keep in milliseconds
        }
    }

    fn print_full_report(&self) {
        let mut report = self.report.lock().unwrap();

        print!("\n\n");

        // Report Details
        Self::print_report_details_full(&report, FIELD_WIDTH);
        println!();

        // convert latencies in ms
        report.latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Request Timings
        Self::print_request_timings_full(&report.latencies);

        println!();
        // Distribution
        Self::print_latency_distribution(&report.latencies);
        println!();

        // Histogram
        Self::print_latency_histogram(&report.latencies);
    }

    /// Print details for full report
    fn print_report_details_full(report: &Report, field_width: usize) {
        println!("{:<field_width$}{}", "Hostname:", report.host, field_width = field_width);
        println!("{:<field_width$}{}", "Port:", report.port, field_width = field_width);
        println!(
            "{:<field_width$}{}\n",
            "Server Software:",
            report.server_software,
            field_width = field_width
        );

        println!(
            "{:<field_width$}{}",
            "Completed Requests:",
            report.completed_requests,
            field_width = field_width
        );
        if report.failed_requests > 0 {
            println!(
                "{:<field_width$}{} (including timeouts)",
                "Failed Requests:",
                report.failed_requests,
                field_width = field_width
            );
            println!("{:<field_width$}{}", "Timeouts:", report.timeouts, field_width = field_width);
        }
        if report.non_2xx_responses > 0 {
            println!(
                "{:<field_width$}{}",
                "Non 2.x.x Responses:",
                report.non_2xx_responses,
                field_width = field_width
            );
        }
        println!(
            "{:<field_width$}{:.2}",
            "Requests/sec:",
            (report.completed_requests as f64) / report.duration.as_secs_f64(),
            field_width = field_width
        );
        println!(
            "{:<field_width$}{:.4} KB",
            "Total HTML Read:",
            report.total_html_read,
            field_width = field_width
        );
        println!(
            "{:<field_width$}{:.2}s",
            "Total Time Taken:",
            report.duration.as_secs_f64(),
            field_width = field_width
        );
    }

    /// Print request timings for full report
    fn print_request_timings_full(latencies_ms: &Vec<f64>) {
        // Calculate min, max, and average
        let min = latencies_ms.first().cloned().unwrap_or(0.0);
        let max = latencies_ms.last().cloned().unwrap_or(0.0);
        let avg = latencies_ms.iter().copied().sum::<f64>() / (latencies_ms.len() as f64);

        // Print in a single row with formatting
        println!("Time Taken for Requests:");
        println!(" {:<12} {:<12} {:<12}", "Min (ms)", "Avg (ms)", "Max (ms)");
        println!(" {:<12.2} {:<12.2} {:<12.2}", min, avg, max);
    }

    fn print_latency_distribution(latencies_ms: &Vec<f64>) {
        if latencies_ms.len() == 0 {
            return; // no requests were sent
        }

        // calculate percentile
        let percentile = |p: f64| -> f64 {
            let idx = ((p / 100.0) * (latencies_ms.len() as f64)) as usize;
            latencies_ms[idx.min(latencies_ms.len() - 1)]
        };

        // get the required percentiles
        let p50 = percentile(50.0);
        let p75 = percentile(75.0);
        let p90 = percentile(90.0);
        let p99 = percentile(99.0);

        // Print results
        println!("Latency Distribution:");
        println!(" 50%    {:.2} ms", p50);
        println!(" 75%    {:.2} ms", p75);
        println!(" 90%    {:.2} ms", p90);
        println!(" 99%    {:.2} ms", p99);
    }

    /// For printing latency histogram
    fn print_latency_histogram(latencies_ms: &Vec<f64>) {
        if latencies_ms.is_empty() {
            return; // no requests were sent
        }

        let max = latencies_ms.last().copied().unwrap_or(0.0);
        let bucket_size = max / (BUCKET_COUNT as f64);

        let mut histogram = vec![0; BUCKET_COUNT]; // using vector for better readability
        for &latency in latencies_ms {
            let bucket = (latency / bucket_size).min((BUCKET_COUNT - 1) as f64) as usize;
            histogram[bucket] += 1;
        }

        println!("{:<15} {:<15} {:>10}", "Range (ms)", "Upper Bound", "Requests");

        for (i, &count) in histogram.iter().enumerate() {
            let lower_bound = (i as f64) * bucket_size;
            let upper_bound = ((i as f64) + 1.0) * bucket_size;
            println!("{:<15.2} {:<15.2} {:>10}", lower_bound, upper_bound, count);
        }
    }
}
