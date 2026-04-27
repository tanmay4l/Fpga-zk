use anyhow::Result;
use ark_bls12_381::{Fr, G1Affine};
use ark_ec::CurveGroup;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fpga_zk::MSMAccelerator;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use tokio::task;

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

fn handle_client(mut stream: TcpStream) -> Result<()> {
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        let request: MSMRequest = serde_json::from_slice(&buffer[..n])?;
        let response = compute_msm(request);
        let response_json = serde_json::to_vec(&response)?;

        stream.write_all(&response_json)?;
    }

    Ok(())
}

fn compute_msm(request: MSMRequest) -> MSMResponse {
    match deserialize_points_and_scalars(&request) {
        Ok((points, scalars)) => {
            let accel = MSMAccelerator::new();
            let result = accel.compute(&points, &scalars);
            let result_affine = result.into_affine();

            let mut buffer = Vec::new();
            match result_affine.serialize_compressed(&mut buffer) {
                Ok(_) => MSMResponse {
                    result: buffer,
                    error: None,
                },
                Err(e) => MSMResponse {
                    result: Vec::new(),
                    error: Some(format!("Serialization error: {}", e)),
                },
            }
        }
        Err(e) => MSMResponse {
            result: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

fn deserialize_points_and_scalars(
    request: &MSMRequest,
) -> Result<(Vec<G1Affine>, Vec<Fr>)> {
    let points: Result<Vec<_>> = request
        .points
        .iter()
        .map(|p| G1Affine::deserialize_compressed(p.as_slice()).map_err(|e| anyhow::anyhow!(e)))
        .collect();

    let scalars: Result<Vec<_>> = request
        .scalars
        .iter()
        .map(|s| Fr::deserialize_compressed(s.as_slice()).map_err(|e| anyhow::anyhow!(e)))
        .collect();

    Ok((points?, scalars?))
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000")?;
    eprintln!("fpga-zk daemon listening on 127.0.0.1:9000");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                task::spawn_blocking(move || {
                    if let Err(e) = handle_client(stream) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}
