use anyhow::{Context, Result};
use ark_bls12_381::{Fr, G1Affine};
use ark_ec::CurveGroup;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fpga_zk::{create_accelerator, MSMAccelerator};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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
) {
    loop {
        let payload = match read_framed(&mut stream).await {
            Ok(p) => p,
            Err(_) => break,
        };

        let response = match process_request(&payload, &*accel) {
            Ok(resp) => resp,
            Err(e) => MSMResponse {
                result: Vec::new(),
                error: Some(e.to_string()),
            },
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
) -> Result<MSMResponse> {
    let request: MSMRequest = serde_json::from_slice(payload)
        .context("deserializing MSMRequest")?;

    let (points, scalars) = deserialize_points_and_scalars(&request)
        .context("deserializing points/scalars")?;

    let result = accel.compute(&points, &scalars);
    let result_affine = result.into_affine();

    let mut buf = Vec::new();
    result_affine.serialize_compressed(&mut buf)
        .context("serializing result")?;

    Ok(MSMResponse { result: buf, error: None })
}

fn deserialize_points_and_scalars(req: &MSMRequest) -> Result<(Vec<G1Affine>, Vec<Fr>)> {
    anyhow::ensure!(
        req.points.len() == req.scalars.len(),
        "points/scalars length mismatch: {} vs {}",
        req.points.len(),
        req.scalars.len()
    );
    let points: Result<Vec<_>> = req
        .points
        .iter()
        .map(|p| G1Affine::deserialize_compressed(p.as_slice())
            .map_err(|e| anyhow::anyhow!("point deser: {}", e)))
        .collect();
    let scalars: Result<Vec<_>> = req
        .scalars
        .iter()
        .map(|s| Fr::deserialize_compressed(s.as_slice())
            .map_err(|e| anyhow::anyhow!("scalar deser: {}", e)))
        .collect();
    Ok((points?, scalars?))
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = env::var("FPGA_ZK_ADDR").unwrap_or_else(|_| "127.0.0.1:9000".to_string());

    let accel: Arc<dyn MSMAccelerator> = Arc::from(create_accelerator(true));
    eprintln!("fpga-zk daemon: accelerator = {}", accel.name());

    let listener = TcpListener::bind(&addr).await
        .with_context(|| format!("binding to {}", addr))?;
    eprintln!("fpga-zk daemon listening on {}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        eprintln!("connection from {}", peer);
        let accel = Arc::clone(&accel);
        tokio::spawn(async move {
            handle_client(stream, accel).await;
        });
    }
}
