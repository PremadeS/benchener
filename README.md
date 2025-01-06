# **benchener**

*benchener* is a high-performance HTTP benchmarking tool developed in [Rust](). Built on the [isahc](https://docs.rs/isahc/latest/isahc/) crate, it supports both HTTP/1.1 and HTTP/2

# **Usage**

```bash
benchener -n1000 -c100 -t2 -s https://www.nayaraasta.com
```

This command:

* Sends **1,000 requests** to https://www.nayaraasta.com 
* Sends **100 requests concurrently**
* Uses **2 threads**
* Displays a **summarized** output

***Note:*** *The actual thread count will exceed the specified number because `isahc` spawns 2 extra threads, and 1 thread is used for timing. The value set by `-t` is passed to `worker_threads()` in the `tokio::runtime::builder`*

## **Output**

```bash
Sending 1000 request(s) to https://www.nayaraasta.com
using 2 thread(s) and 100 connection(s)
Please be patient..
Completed requests: 1000

Sent 1000 requests in 1.23s, 3684.570KB read (html)
Latnecy Stats:
 Avg        Min        Max        Stdev     
 123.45ms   50.12ms    456.78ms   78.90ms     
Latency Distribution:
 50%     115.67 ms
 75%     180.23 ms
 90%     280.98 ms
 99%     400.12 ms
Request(s) per sec:   813.008
Transfer per sec:    2995.585 KB (html)
```

## CLI Arguments

```bash
Usage: benchener [OPTIONS] <URL>

benchener powered by nayaraasta

Options:
  -n, --requests           <N>  Number of requests (Default: 10)
  -d, --duration           <D>  Test duration
  -c, --concurrency        <N>  Concurrent requests (Default: 1)
  -t, --threads            <N>  Number of threads (Default: 1)
  -T, --timeout            <D>  Request timeout (Default: 25s)
  -C, --connection-timeout <D>  Connection timeout (Default: 20s)
  -s                            Summarize output
  -h, --help                    Print help (this)
  -v, --version                 Print version

Arguments:
  <URL>                         URL to test

Durations can be specified like: 10s, 1m, 1h
The test ends when either -n or -d completes. (if both are given)
```

# **Installation**

## **Linux**

### **Debian-Based Distributions (Ubuntu, Mint, etc.)**

1. Download the `.deb` package:

```bash
wget https://www.nayaraasta.com/benchener/linux/benchener_1.0.0-1_amd64.deb
```

2. Install it

```bash
sudo dpkg -i benchener_1.0.0-1_amd64.deb
```

### **RPM-Based Distributions** (Fedora, CentOS, etc.)

1. Download the `.rpm` package:

```bash
wget https://www.nayaraasta.com/benchener/linux/benchener-1.0.0-1.x86_64.rpm
```

2. Install with `dnf` (Fedora, CentOS 8+):

```bash
sudo dnf install benchener-1.0.0-1.x86_64.rpm
```

*For older systems (CentOS 7/RHEL 7), use `yum` instead:*

```bash
sudo yum install benchener-1.0.0-1.x86_64.rpm
```

## **Others (Windows, macOS, Linux)**

For other platforms, install **benchener** from source using `cargo`.

* Make sure to have **Rust** and **Cargo** installed. [Rust installation page](https://www.rust-lang.org/tools/install)

```bash
cargo install --git https://github.com/PremadeS/benchener
```

*GUI coming soon*