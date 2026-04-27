use anyhow::{Context, Result};
use ark_bls12_381::{Fr as FrBls, G1Affine as G1AffineBls};
use ark_bn254::{Fr as FrBn, G1Affine as G1AffineBn, Bn254};
use ark_ec::CurveGroup;
use ark_groth16::Proof;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fpga_zk::{create_accelerator, MSMAccelerator, groth16};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
enum Request {
    #[serde(rename = "msm")]
    Msm(MSMRequest),
    #[serde(rename = "prove")]
    Prove(ProveRequest),
}

#[derive(Serialize, Deserialize)]
struct MSMRequest {
    points: Vec<Vec<u8>>,
    scalars: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
struct ProveRequest {
    points: Vec<Vec<u8>>, 
    scalars: Vec<Vec<u8>>, 
}

#[derive(Serialize, Deserialize)]
struct MSMResponse {
    result: Vec<u8>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ProveResponse {
    result: Vec<u8>,
    proof_a: Vec<u8>,
    proof_b: Vec<u8>,
    proof_c: Vec<u8>,
    error: Option<String>,
}

async fn read_framed(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.context("reading length prefix")?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 4 * 1024 * 1024 {
        anyhow::bail!("message too large: {} bytes", len);
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await.context("reading payload")?;
    Ok(payload)
}

async fn write_framed(stream: &mut TcpStream, payload: &[u8]) -> Result<()> {
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(payload).await?;
    Ok(())
}

async fn handle_client(
    mut stream: TcpStream,
    accel: Arc<dyn MSMAccelerator>,
    pk_path: PathBuf,
) {
    loop {
        let payload = match read_framed(&mut stream).await {
            Ok(p) => p,
            Err(_) => break,
        };

        let response = match process_request(&payload, &*accel, &pk_path) {
            Ok(resp) => resp,
            Err(e) => {
                let resp_json = if payload.windows(5).any(|w| w == b"prove") {
                    serde_json::to_vec(&ProveResponse {
                        result: Vec::new(),
                        proof_a: Vec::new(),
                        proof_b: Vec::new(),
                        proof_c: Vec::new(),
                        error: Some(e.to_string()),
                    })
                } else {
                    serde_json::to_vec(&MSMResponse {
                        result: Vec::new(),
                        error: Some(e.to_string()),
                    })
                };
                if let Ok(j) = resp_json {
                    let _ = write_framed(&mut stream, &j).await;
                }
                continue;
            }
        };

        let response_json = match serde_json::to_vec(&response) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("serialization error: {}", e);
                break;
            }
        };

        if let Err(e) = write_framed(&mut stream, &response_json).await {
            eprintln!("write error: {}", e);
            break;
        }
    }
}

fn process_request(
    payload: &[u8],
    accel: &dyn MSMAccelerator,
    pk_path: &std::path::Path,
) -> Result<serde_json::Value> {
    let request: Request = serde_json::from_slice(payload)
        .context("deserializing request")?;

    match request {
        Request::Msm(req) => {
            let (points, scalars) = deserialize_bls12381_points_and_scalars(&req)
                .context("deserializing BLS12-381 points/scalars")?;

            let result = accel.compute(&points, &scalars);
            let result_affine = result.into_affine();

            let mut buf = Vec::new();
            result_affine.serialize_compressed(&mut buf)
                .context("serializing result")?;

            let response = MSMResponse { result: buf, error: None };
            Ok(serde_json::to_value(response)?)
        }
        Request::Prove(req) => {
            let (points, scalars) = deserialize_bn254_points_and_scalars(&req)
                .context("deserializing BN254 points/scalars")?;

            anyhow::ensure!(
                !points.is_empty() && points.len() == scalars.len(),
                "invalid prove request: {} points, {} scalars",
                points.len(),
                scalars.len()
            );

            // Load proving key
            let pk = groth16::load_proving_key(pk_path)
                .map_err(|e| anyhow::anyhow!("loading proving key: {}", e))?;

            // Compute MSM result
            let result: G1AffineBn = points
                .iter()
                .zip(&scalars)
                .map(|(p, s)| ark_bn254::G1Projective::from(*p) * s)
                .sum::<ark_bn254::G1Projective>()
                .into();

            // Generate proof
            let mut rng = rand::rngs::OsRng;
            let proof: Proof<Bn254> = groth16::prove(&pk, &points, &scalars, result, &mut rng)
                .map_err(|e| anyhow::anyhow!("proof generation: {}", e))?;

            // Serialize result and proof components
            let mut result_bytes = Vec::new();
            result.serialize_compressed(&mut result_bytes)
                .context("serializing result")?;

            let mut proof_a_bytes = Vec::new();
            proof.a.serialize_compressed(&mut proof_a_bytes)
                .context("serializing proof.a")?;

            let mut proof_b_bytes = Vec::new();
            proof.b.serialize_compressed(&mut proof_b_bytes)
                .context("serializing proof.b")?;

            let mut proof_c_bytes = Vec::new();
            proof.c.serialize_compressed(&mut proof_c_bytes)
                .context("serializing proof.c")?;

            let response = ProveResponse {
                result: result_bytes,
                proof_a: proof_a_bytes,
                proof_b: proof_b_bytes,
                proof_c: proof_c_bytes,
                error: None,
            };

            Ok(serde_json::to_value(response)?)
        }
    }
}

fn deserialize_bls12381_points_and_scalars(
    req: &MSMRequest,
) -> Result<(Vec<G1AffineBls>, Vec<FrBls>)> {
    anyhow::ensure!(
        req.points.len() == req.scalars.len(),
        "points/scalars length mismatch: {} vs {}",
        req.points.len(),
        req.scalars.len()
    );
    let points: Result<Vec<_>> = req
        .points
        .iter()
        .map(|p| G1AffineBls::deserialize_compressed(p.as_slice())
            .map_err(|e| anyhow::anyhow!("point deser: {}", e)))
        .collect();
    let scalars: Result<Vec<_>> = req
        .scalars
        .iter()
        .map(|s| FrBls::deserialize_compressed(s.as_slice())
            .map_err(|e| anyhow::anyhow!("scalar deser: {}", e)))
        .collect();
    Ok((points?, scalars?))
}

fn deserialize_bn254_points_and_scalars(
    req: &ProveRequest,
) -> Result<(Vec<G1AffineBn>, Vec<FrBn>)> {
    anyhow::ensure!(
        req.points.len() == req.scalars.len(),
        "points/scalars length mismatch: {} vs {}",
        req.points.len(),
        req.scalars.len()
    );
    let points: Result<Vec<_>> = req
        .points
        .iter()
        .map(|p| G1AffineBn::deserialize_compressed(p.as_slice())
            .map_err(|e| anyhow::anyhow!("BN254 point deser: {}", e)))
        .collect();
    let scalars: Result<Vec<_>> = req
        .scalars
        .iter()
        .map(|s| FrBn::deserialize_compressed(s.as_slice())
            .map_err(|e| anyhow::anyhow!("BN254 scalar deser: {}", e)))
        .collect();
    Ok((points?, scalars?))
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = env::var("FPGA_ZK_ADDR").unwrap_or_else(|_| "127.0.0.1:9000".to_string());
    let pk_path = env::var("FPGA_ZK_PK")
        .unwrap_or_else(|_| "proving_key.bin".to_string());

    let accel: Arc<dyn MSMAccelerator> = Arc::from(create_accelerator(true));
    eprintln!("fpga-zk daemon: accelerator = {}", accel.name());

    let listener = TcpListener::bind(&addr).await
        .with_context(|| format!("binding to {}", addr))?;
    eprintln!("fpga-zk daemon listening on {}", addr);
    eprintln!("fpga-zk daemon proving key path: {}", pk_path);

    let pk_path = PathBuf::from(pk_path);

    loop {
        let (stream, peer) = listener.accept().await?;
        eprintln!("connection from {}", peer);
        let accel = Arc::clone(&accel);
        let pk_path = pk_path.clone();
        tokio::spawn(async move {
            handle_client(stream, accel, pk_path).await;
        });
    }
}
