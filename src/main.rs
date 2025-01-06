mod config;
mod runner;
mod report;

use config::Config;
use runner::Runner;

fn main() {
    let config = Config::parse();

    let runner = Runner::new(config);

    let runner_clone = runner.clone();

    ctrlc
        ::set_handler(move || {
            runner_clone.print_report();
            std::process::exit(0);
        })
        .expect("Error setting Ctrl+C handler");

    let result = runner.run();
    match result {
        Ok(res) => { res.print_report() }
        Err(err) => { eprintln!("{}", err) }
    }
}
