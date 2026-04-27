use ark_bn254::{Fr, G1Affine, G1Projective, Bn254};
use ark_ff::UniformRand;
use ark_groth16::Groth16;
use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fpga_zk::groth16::{self, MSMCircuit};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[derive(Serialize, Deserialize)]
struct ProveRequest {
    op: String,
    points: Vec<Vec<u8>>,
    scalars: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
struct ProveResponse {
    result: Vec<u8>,
    proof_a: Vec<u8>,
    proof_b: Vec<u8>,
    proof_c: Vec<u8>,
    #[serde(default)]
    error: Option<String>,
}

fn setup_daemon_and_keys(temp_dir: &TempDir) -> PathBuf {
    let pk_path = temp_dir.path().join("proving_key.bin");

    let mut rng = OsRng;
    let n = 4;
    let (pk, vk) = groth16::generate_keys(n, &mut rng).expect("key generation failed");

    groth16::save_proving_key(&pk, &pk_path).expect("save pk failed");
    groth16::save_verifying_key(&vk, temp_dir.path().join("verifying_key.bin"))
        .expect("save vk failed");

    pk_path
}

#[test]
fn test_groth16_prove_verify_local() {
    let mut rng = OsRng;
    let n = 4;

    let points: Vec<_> = (0..n)
        .map(|_| G1Projective::rand(&mut rng).into())
        .collect();
    let scalars: Vec<_> = (0..n).map(|_| Fr::rand(&mut rng)).collect();
    let result: G1Affine = points
        .iter()
        .zip(&scalars)
        .map(|(p, s)| G1Projective::from(*p) * s)
        .sum::<G1Projective>()
        .into();

    let scalar_sum: Fr = scalars.iter().sum();

    let (pk, vk) = Groth16::<Bn254>::setup(MSMCircuit::empty(n), &mut rng)
        .expect("setup failed");

    let circuit = MSMCircuit {
        points: points.clone(),
        result,
        scalars: scalars.clone(),
    };

    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).expect("prove failed");

    let verified = Groth16::<Bn254>::verify(&vk, &vec![scalar_sum], &proof)
        .expect("verify failed");

    assert!(verified, "local verification failed");
}

#[test]
fn test_daemon_prove_request() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pk_path = setup_daemon_and_keys(&temp_dir);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    // Spawn daemon in background
    let pk_path_clone = pk_path.clone();
    let daemon_handle = thread::spawn(move || {
        run_daemon_once(listener, pk_path_clone)
    });

    // Give daemon time to start
    thread::sleep(Duration::from_millis(100));

    // Send prove request
    let mut rng = OsRng;
    let n = 4;

    let points: Vec<_> = (0..n)
        .map(|_| G1Projective::rand(&mut rng).into())
        .collect();
    let scalars: Vec<_> = (0..n).map(|_| Fr::rand(&mut rng)).collect();
    let result: G1Affine = points
        .iter()
        .zip(&scalars)
        .map(|(p, s)| G1Projective::from(*p) * s)
        .sum::<G1Projective>()
        .into();


    let points_bytes: Vec<Vec<u8>> = points
        .iter()
        .map(|p: &G1Affine| {
            let mut buf = Vec::new();
            p.serialize_compressed(&mut buf).expect("serialize");
            buf
        })
        .collect();

    let scalars_bytes: Vec<Vec<u8>> = scalars
        .iter()
        .map(|s: &Fr| {
            let mut buf = Vec::new();
            s.serialize_compressed(&mut buf).expect("serialize");
            buf
        })
        .collect();

    let request = ProveRequest {
        op: "prove".to_string(),
        points: points_bytes,
        scalars: scalars_bytes,
    };

    let request_json = serde_json::to_vec(&request).expect("json serialize");
    let mut request_framed = Vec::new();
    request_framed.extend_from_slice(&(request_json.len() as u32).to_le_bytes());
    request_framed.extend_from_slice(&request_json);


    let mut stream = std::net::TcpStream::connect(addr).expect("connect failed");
    stream
        .write_all(&request_framed)
        .expect("write failed");

   
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).expect("read len failed");
    let len = u32::from_le_bytes(len_buf) as usize;

    let mut response_buf = vec![0u8; len];
    stream.read_exact(&mut response_buf).expect("read response failed");

    let response: ProveResponse = serde_json::from_slice(&response_buf).expect("parse response");

    assert!(
        response.error.is_none(),
        "daemon error: {:?}",
        response.error
    );
    assert!(!response.result.is_empty(), "no result");
    assert!(!response.proof_a.is_empty(), "no proof.a");
    assert!(!response.proof_b.is_empty(), "no proof.b");
    assert!(!response.proof_c.is_empty(), "no proof.c");


    let result_bytes = {
        let mut buf = Vec::new();
        result.serialize_compressed(&mut buf).expect("serialize");
        buf
    };
    assert_eq!(response.result, result_bytes, "result mismatch");

    daemon_handle.join().expect("daemon thread panic");
}

fn run_daemon_once(listener: TcpListener, pk_path: PathBuf) {
    if let Ok((mut stream, _)) = listener.accept() {

        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() {
            return;
        }

        let len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        if stream.read_exact(&mut payload).is_err() {
            return;
        }

        let request: ProveRequest = match serde_json::from_slice(&payload) {
            Ok(r) => r,
            Err(e) => {
                let error_response = ProveResponse {
                    result: Vec::new(),
                    proof_a: Vec::new(),
                    proof_b: Vec::new(),
                    proof_c: Vec::new(),
                    error: Some(format!("parse error: {}", e)),
                };
                let _ = send_response(&mut stream, &error_response);
                return;
            }
        };


        let response = process_prove_request(&request, &pk_path);
        let _ = send_response(&mut stream, &response);
    }
}

fn process_prove_request(request: &ProveRequest, pk_path: &std::path::Path) -> ProveResponse {
    // Deserialize points and scalars
    let points: Vec<G1Affine> = match request
        .points
        .iter()
        .map(|p| G1Affine::deserialize_compressed(p.as_slice()))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(p) => p,
        Err(e) => {
            return ProveResponse {
                result: Vec::new(),
                proof_a: Vec::new(),
                proof_b: Vec::new(),
                proof_c: Vec::new(),
                error: Some(format!("deserialize points: {}", e)),
            }
        }
    };

    let scalars: Vec<Fr> = match request
        .scalars
        .iter()
        .map(|s| Fr::deserialize_compressed(s.as_slice()))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(s) => s,
        Err(e) => {
            return ProveResponse {
                result: Vec::new(),
                proof_a: Vec::new(),
                proof_b: Vec::new(),
                proof_c: Vec::new(),
                error: Some(format!("deserialize scalars: {}", e)),
            }
        }
    };

    // Load proving key
    let pk = match groth16::load_proving_key(pk_path) {
        Ok(pk) => pk,
        Err(e) => {
            return ProveResponse {
                result: Vec::new(),
                proof_a: Vec::new(),
                proof_b: Vec::new(),
                proof_c: Vec::new(),
                error: Some(format!("load pk: {}", e)),
            }
        }
    };

    // Compute result
    let result: G1Affine = points
        .iter()
        .zip(&scalars)
        .map(|(p, s)| G1Projective::from(*p) * s)
        .sum::<G1Projective>()
        .into();

    // Generate proof
    let mut rng = OsRng;
    let proof = match groth16::prove(&pk, &points, &scalars, result, &mut rng) {
        Ok(p) => p,
        Err(e) => {
            return ProveResponse {
                result: Vec::new(),
                proof_a: Vec::new(),
                proof_b: Vec::new(),
                proof_c: Vec::new(),
                error: Some(format!("prove: {}", e)),
            }
        }
    };

    // Serialize everything
    let mut result_bytes = Vec::new();
    let _ = result.serialize_compressed(&mut result_bytes);

    let mut proof_a_bytes = Vec::new();
    let _ = proof.a.serialize_compressed(&mut proof_a_bytes);

    let mut proof_b_bytes = Vec::new();
    let _ = proof.b.serialize_compressed(&mut proof_b_bytes);

    let mut proof_c_bytes = Vec::new();
    let _ = proof.c.serialize_compressed(&mut proof_c_bytes);

    ProveResponse {
        result: result_bytes,
        proof_a: proof_a_bytes,
        proof_b: proof_b_bytes,
        proof_c: proof_c_bytes,
        error: None,
    }
}

fn send_response(stream: &mut std::net::TcpStream, response: &ProveResponse) -> std::io::Result<()> {
    let response_json = serde_json::to_vec(&response)?;
    let len = response_json.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&response_json)?;
    Ok(())
}
