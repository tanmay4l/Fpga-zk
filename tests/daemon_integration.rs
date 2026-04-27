use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::UniformRand;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fpga_zk::PippengerMSM;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize)]
struct MSMRequest {
    points: Vec<Vec<u8>>,
    scalars: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
struct MSMResponse {
    result: Vec<u8>,
    error: Option<String>,
}

fn write_framed(stream: &mut TcpStream, payload: &[u8]) {
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes()).unwrap();
    stream.write_all(payload).unwrap();
}

fn read_framed(stream: &mut TcpStream) -> Vec<u8> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).unwrap();
    payload
}

fn spawn_daemon(addr: &str) -> Child {
    let bin = std::env::var("CARGO_BIN_EXE_fpga_zk_daemon")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_fpga-zk-daemon"))
        .unwrap_or_else(|_| "./target/debug/fpga-zk-daemon".to_string());
    Command::new(bin)
        .env("FPGA_ZK_ADDR", addr)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn fpga-zk-daemon")
}

fn wait_for_daemon(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[ignore]
fn daemon_tcp_roundtrip_4_points() {
    let addr = "127.0.0.1:19000";
    let mut daemon = spawn_daemon(addr);

    if !wait_for_daemon(addr, Duration::from_secs(5)) {
        daemon.kill().ok();
        panic!("daemon did not start within 5 seconds");
    }

    let mut stream = TcpStream::connect(addr).expect("connect to daemon");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

    let mut rng = OsRng;
    let n = 4;
    let points: Vec<G1Affine> = (0..n)
        .map(|_| G1Projective::rand(&mut rng).into_affine())
        .collect();
    let scalars: Vec<Fr> = (0..n).map(|_| Fr::rand(&mut rng)).collect();

    let points_bytes: Vec<Vec<u8>> = points.iter().map(|p| {
        let mut buf = Vec::new();
        p.serialize_compressed(&mut buf).unwrap();
        buf
    }).collect();
    let scalars_bytes: Vec<Vec<u8>> = scalars.iter().map(|s| {
        let mut buf = Vec::new();
        s.serialize_compressed(&mut buf).unwrap();
        buf
    }).collect();

    let request = MSMRequest { points: points_bytes, scalars: scalars_bytes };
    let request_json = serde_json::to_vec(&request).unwrap();

    write_framed(&mut stream, &request_json);
    let response_bytes = read_framed(&mut stream);

    let response: MSMResponse = serde_json::from_slice(&response_bytes)
        .expect("parse MSMResponse");

    assert!(
        response.error.is_none(),
        "daemon returned error: {:?}", response.error
    );
    assert_eq!(response.result.len(), 48, "G1Affine compressed is 48 bytes");

    let got = G1Affine::deserialize_compressed(response.result.as_slice())
        .expect("deserialize daemon result");

    let pippenger = PippengerMSM::new();
    let expected = pippenger.compute(&points, &scalars).into_affine();

    assert_eq!(got, expected, "daemon result must match local Pippenger");

    daemon.kill().ok();
}

#[test]
#[ignore]
fn daemon_tcp_roundtrip_8_points() {
    let addr = "127.0.0.1:19001";
    let mut daemon = spawn_daemon(addr);
    if !wait_for_daemon(addr, Duration::from_secs(5)) {
        daemon.kill().ok();
        panic!("daemon did not start");
    }

    let mut stream = TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

    let mut rng = OsRng;
    let n = 8;
    let points: Vec<G1Affine> = (0..n)
        .map(|_| G1Projective::rand(&mut rng).into_affine())
        .collect();
    let scalars: Vec<Fr> = (0..n).map(|_| Fr::rand(&mut rng)).collect();

    let points_bytes: Vec<Vec<u8>> = points.iter().map(|p| {
        let mut buf = Vec::new(); p.serialize_compressed(&mut buf).unwrap(); buf
    }).collect();
    let scalars_bytes: Vec<Vec<u8>> = scalars.iter().map(|s| {
        let mut buf = Vec::new(); s.serialize_compressed(&mut buf).unwrap(); buf
    }).collect();

    let request_json = serde_json::to_vec(&MSMRequest {
        points: points_bytes, scalars: scalars_bytes,
    }).unwrap();

    write_framed(&mut stream, &request_json);
    let response_bytes = read_framed(&mut stream);
    let response: MSMResponse = serde_json::from_slice(&response_bytes).unwrap();

    assert!(response.error.is_none(), "error: {:?}", response.error);

    let got = G1Affine::deserialize_compressed(response.result.as_slice()).unwrap();
    let expected = PippengerMSM::new().compute(&points, &scalars).into_affine();
    assert_eq!(got, expected);

    daemon.kill().ok();
}

#[test]
#[ignore]
fn daemon_tcp_rejects_malformed_request() {
    let addr = "127.0.0.1:19002";
    let mut daemon = spawn_daemon(addr);
    if !wait_for_daemon(addr, Duration::from_secs(5)) {
        daemon.kill().ok();
        panic!("daemon did not start");
    }

    let mut stream = TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

    let garbage = b"not valid json {{{";
    write_framed(&mut stream, garbage);
    let response_bytes = read_framed(&mut stream);
    let response: MSMResponse = serde_json::from_slice(&response_bytes).unwrap();

    assert!(response.error.is_some(), "should return an error for malformed JSON");
    assert!(response.result.is_empty());

    daemon.kill().ok();
}
