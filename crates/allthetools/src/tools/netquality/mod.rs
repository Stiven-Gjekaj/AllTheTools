//! `att netquality`: measure the speed and the responsiveness of the network.
//!
//! macOS has `networkQuality`, which measures the download, the upload and
//! the responsiveness of the network while it is busy. Linux and Windows
//! have no such command. This tool does the same three measures on each
//! host, with the speed test server of Cloudflare.
//!
//! The responsiveness is in round trips per minute (RPM): the number of
//! request and answer pairs that fit into one minute while the network is
//! full. This tool measures each round trip on a new connection, from the
//! request to the end of the answer. `networkQuality` also measures round
//! trips inside the connections that carry the load, and on a link that
//! queues each connection by itself those round trips are much slower. So
//! the RPM of this tool can be much higher than the RPM of `networkQuality`
//! on the same link. Compare a number of this tool with another number of
//! this tool.
//!
//! The speed is the speed to the server. Two servers in two places give two
//! speeds on the same link.
//!
//! The test uses plain HTTP. It sends and receives only zeros and random
//! bytes, and plain HTTP needs no TLS library in the binary.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Serialize;

pub mod parse;

/// Measure the download, the upload and the responsiveness of the network.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The time of the download and of the upload, each: seconds, or a
    /// number with s or m.
    #[arg(short, long, default_value = "8s", value_name = "TIME")]
    pub time: String,

    /// The number of connections that carry the data at the same time.
    #[arg(short, long, default_value_t = 8)]
    pub connections: usize,

    /// The server. It must give the speed test API of Cloudflare, with
    /// /__down and /__up.
    #[arg(long, default_value = "speed.cloudflare.com")]
    pub server: String,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

/// The size of one download request. A connection that finishes one asks
/// for the next. Cloudflare refuses a request of 100 MB with status 403.
const DOWN_BYTES: u64 = 25_000_000;
/// The size of one upload request.
const UP_BYTES: u64 = 25_000_000;
/// The time between two probes while the network is full.
const PROBE_EVERY: Duration = Duration::from_millis(250);
const IDLE_PROBES: usize = 5;

#[derive(Debug, Serialize)]
struct Report {
    server: String,
    address: String,
    idle_latency_ms: Option<f64>,
    download_mbps: f64,
    download_latency_ms: Option<f64>,
    upload_mbps: f64,
    upload_latency_ms: Option<f64>,
    responsiveness_rpm: Option<f64>,
    probes: usize,
}

#[derive(Clone)]
struct Server {
    host: String,
    address: SocketAddr,
}

pub fn run(args: Args) -> Result<i32> {
    let time = crate::time::duration(&args.time)?;
    anyhow::ensure!(
        time >= Duration::from_secs(2),
        "give a time of 2 seconds or more"
    );
    anyhow::ensure!(
        (1..=64).contains(&args.connections),
        "give from 1 to 64 connections"
    );
    let address = (args.server.as_str(), 80)
        .to_socket_addrs()
        .with_context(|| format!("could not find the address of {}", args.server))?
        .next()
        .with_context(|| format!("{} has no address", args.server))?;
    let server = Server {
        host: args.server.clone(),
        address,
    };

    note(
        &args,
        &format!(
            "Measuring the idle latency to {} ({address})...",
            server.host
        ),
    );
    let mut idle = Vec::new();
    for _ in 0..IDLE_PROBES {
        if let Some(ms) = probe(&server) {
            idle.push(ms);
        }
    }
    if idle.is_empty() {
        bail!(
            "{} did not answer. Check the connection to the internet",
            server.host
        );
    }

    note(&args, "Measuring the download...");
    let (down, down_probes) = phase(&server, args.connections, time, Direction::Down);
    note(&args, "Measuring the upload...");
    let (up, up_probes) = phase(&server, args.connections, time, Direction::Up);

    if down == 0.0 && up == 0.0 {
        bail!(
            "{} answered the probes, and moved no data. It may refuse the requests of the speed test",
            server.host
        );
    }
    let loaded: Vec<f64> = down_probes.iter().chain(&up_probes).copied().collect();
    let loaded_median = parse::median(&loaded);
    let report = Report {
        server: server.host.clone(),
        address: address.ip().to_string(),
        idle_latency_ms: parse::median(&idle),
        download_mbps: down,
        download_latency_ms: parse::median(&down_probes),
        upload_mbps: up,
        upload_latency_ms: parse::median(&up_probes),
        responsiveness_rpm: loaded_median.and_then(parse::rpm),
        probes: loaded.len(),
    };

    if args.json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print(&report);
    }
    Ok(0)
}

fn note(args: &Args, text: &str) {
    if !args.json {
        eprintln!("{text}");
    }
}

fn print(report: &Report) {
    let ms = |v: Option<f64>| v.map_or("-".to_owned(), |v| format!("{v:.1} ms"));
    let rows = vec![
        vec![
            "Server".to_owned(),
            format!("{} ({})", report.server, report.address),
        ],
        vec!["Idle latency".to_owned(), ms(report.idle_latency_ms)],
        vec![
            "Download".to_owned(),
            format!(
                "{:.1} Mbps, latency {} while busy",
                report.download_mbps,
                ms(report.download_latency_ms)
            ),
        ],
        vec![
            "Upload".to_owned(),
            format!(
                "{:.1} Mbps, latency {} while busy",
                report.upload_mbps,
                ms(report.upload_latency_ms)
            ),
        ],
        vec![
            "Responsiveness".to_owned(),
            report.responsiveness_rpm.map_or("-".to_owned(), |r| {
                format!("{r:.0} RPM, from {} round trips while busy", report.probes)
            }),
        ],
    ];
    for row in rows {
        println!("{:<16}{}", row[0], row[1]);
    }
}

fn connect(server: &Server) -> std::io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&server.address, Duration::from_secs(5))?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    Ok(stream)
}

/// One round trip on a new connection, from the request to the end of the
/// answer, in milliseconds.
fn probe(server: &Server) -> Option<f64> {
    let mut stream = connect(server).ok()?;
    let start = Instant::now();
    stream
        .write_all(parse::get(&server.host, "/__down?bytes=0").as_bytes())
        .ok()?;
    let stop = AtomicBool::new(false);
    let status = read_response(&mut stream, &stop, &mut |_| {}).ok()?;
    (status == 200).then(|| start.elapsed().as_secs_f64() * 1000.0)
}

/// Read one response, give the count of each part of its body to `count`,
/// and give its status. The read ends early when `stop` turns true.
fn read_response(
    stream: &mut TcpStream,
    stop: &AtomicBool,
    count: &mut dyn FnMut(usize),
) -> std::io::Result<u16> {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut filled = 0;
    let head = loop {
        let read = stream.read(&mut buffer[filled..])?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        filled += read;
        if let Some(head) = parse::head(&buffer[..filled]) {
            break head;
        }
        if filled == buffer.len() {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
    };
    let mut left = head.length.unwrap_or(0);
    let early = (filled - head.size) as u64;
    count(early as usize);
    left = left.saturating_sub(early);
    while left > 0 && !stop.load(Ordering::Relaxed) {
        let want = (left as usize).min(buffer.len());
        match stream.read(&mut buffer[..want]) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(read) => {
                count(read);
                left -= read as u64;
            }
            Err(e) if is_timeout(&e) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(head.status)
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

#[derive(Clone, Copy)]
enum Direction {
    Down,
    Up,
}

/// Fill the network in one direction for `time`, and probe it at the same
/// time. The speed comes from the second half of the time, after the
/// connections have reached their speed.
fn phase(
    server: &Server,
    connections: usize,
    time: Duration,
    direction: Direction,
) -> (f64, Vec<f64>) {
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = Arc::new(AtomicU64::new(0));
    let probes = Arc::new(Mutex::new(Vec::new()));

    let mut workers = Vec::new();
    for _ in 0..connections {
        let (server, stop, bytes) = (server.clone(), stop.clone(), bytes.clone());
        workers.push(thread::spawn(move || match direction {
            Direction::Down => download(&server, &stop, &bytes),
            Direction::Up => upload(&server, &stop, &bytes),
        }));
    }
    let prober = {
        let (server, stop, probes) = (server.clone(), stop.clone(), probes.clone());
        thread::spawn(move || {
            // The first probe waits for the connections to fill the link.
            thread::sleep(Duration::from_secs(1));
            while !stop.load(Ordering::Relaxed) {
                if let Some(ms) = probe(&server)
                    && let Ok(mut list) = probes.lock()
                {
                    list.push(ms);
                }
                thread::sleep(PROBE_EVERY);
            }
        })
    };

    thread::sleep(time / 2);
    let (start_bytes, start) = (bytes.load(Ordering::Relaxed), Instant::now());
    thread::sleep(time / 2);
    let (end_bytes, end) = (bytes.load(Ordering::Relaxed), Instant::now());
    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        let _ = worker.join();
    }
    let _ = prober.join();

    let speed = parse::mbps(end_bytes - start_bytes, end - start);
    let probes = probes.lock().map(|p| p.clone()).unwrap_or_default();
    (speed, probes)
}

fn download(server: &Server, stop: &AtomicBool, bytes: &AtomicU64) {
    let path = format!("/__down?bytes={DOWN_BYTES}");
    while !stop.load(Ordering::Relaxed) {
        let Ok(mut stream) = connect(server) else {
            thread::sleep(Duration::from_millis(200));
            continue;
        };
        while !stop.load(Ordering::Relaxed) {
            if stream
                .write_all(parse::get(&server.host, &path).as_bytes())
                .is_err()
            {
                break;
            }
            let mut count = |n: usize| {
                bytes.fetch_add(n as u64, Ordering::Relaxed);
            };
            match read_response(&mut stream, stop, &mut count) {
                Ok(200) => {}
                // The server refuses the request, and it refuses the next
                // one too. The speed of zero then says so.
                Ok(_) => return,
                Err(_) => break,
            }
        }
    }
}

fn upload(server: &Server, stop: &AtomicBool, bytes: &AtomicU64) {
    let chunk = vec![0u8; 64 * 1024];
    while !stop.load(Ordering::Relaxed) {
        let Ok(mut stream) = connect(server) else {
            thread::sleep(Duration::from_millis(200));
            continue;
        };
        'requests: while !stop.load(Ordering::Relaxed) {
            if stream
                .write_all(parse::post(&server.host, "/__up", UP_BYTES).as_bytes())
                .is_err()
            {
                break;
            }
            let mut left = UP_BYTES;
            while left > 0 {
                if stop.load(Ordering::Relaxed) {
                    break 'requests;
                }
                let size = (left as usize).min(chunk.len());
                match stream.write(&chunk[..size]) {
                    Ok(written) => {
                        bytes.fetch_add(written as u64, Ordering::Relaxed);
                        left -= written as u64;
                    }
                    Err(e) if is_timeout(&e) => continue,
                    Err(_) => break 'requests,
                }
            }
            if read_response(&mut stream, stop, &mut |_| {}).is_err() {
                break;
            }
        }
    }
}
