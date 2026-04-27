
use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::UniformRand;
use ark_serialize::CanonicalSerialize;
use fpga_zk::PippengerMSM;
use rand::rngs::OsRng;
use std::fs::File;
use std::io::Write;

#[derive(Debug)]
struct TestVector {
    name: String,
    num_points: usize,
    points: Vec<G1Affine>,
    scalars: Vec<Fr>,
    expected_result: G1Projective,
}

impl TestVector {
    fn to_systemverilog(&self) -> String {
        let mut sv = format!("// Test: {}\n", self.name);
        sv.push_str(&format!("// Points: {}\n\n", self.num_points));

        // Points in hex
        sv.push_str("// Points (compressed, 48 bytes each)\n");
        for (i, point) in self.points.iter().enumerate() {
            let mut buf = Vec::new();
            point.serialize_compressed(&mut buf).unwrap();
            let hex = buf
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join("");
            sv.push_str(&format!("test_points[{}] = 256'h{};\n", i, hex));
        }

        // Scalars in hex
        sv.push_str("\n// Scalars\n");
        for (i, scalar) in self.scalars.iter().enumerate() {
            let mut buf = Vec::new();
            scalar.serialize_compressed(&mut buf).unwrap();
            let hex = buf
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join("");
            sv.push_str(&format!("test_scalars[{}] = 256'h{};\n", i, hex));
        }

        sv.push_str("\n// Expected result\n");
        let mut buf = Vec::new();
        self.expected_result
            .into_affine()
            .serialize_compressed(&mut buf)
            .unwrap();
        let hex = buf
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");
        sv.push_str(&format!("expected_result = 256'h{};\n", hex));

        sv
    }

    fn verify_software(&self) -> bool {
        let pippenger = PippengerMSM::new();
        let computed = pippenger.compute(&self.points, &self.scalars);

        if computed == self.expected_result {
            println!("✓ {} - PASS", self.name);
            true
        } else {
            println!("✗ {} - FAIL", self.name);
            println!("  Expected: {:?}", self.expected_result);
            println!("  Got:      {:?}", computed);
            false
        }
    }
}

fn generate_test_vectors() -> Vec<TestVector> {
    let mut tests = Vec::new();
    let mut rng = OsRng;

    {
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::rand(&mut rng);
        let expected = G1Projective::from(point) * scalar;

        tests.push(TestVector {
            name: "Single Point MSM".to_string(),
            num_points: 1,
            points: vec![point],
            scalars: vec![scalar],
            expected_result: expected,
        });
    }

    {
        let points: Vec<_> = (0..2)
            .map(|_| G1Projective::rand(&mut rng).into_affine())
            .collect();
        let scalars: Vec<_> = (0..2).map(|_| Fr::rand(&mut rng)).collect();
        let expected: G1Projective = points
            .iter()
            .zip(&scalars)
            .map(|(p, s)| G1Projective::from(*p) * s)
            .sum();

        tests.push(TestVector {
            name: "Two Point MSM".to_string(),
            num_points: 2,
            points,
            scalars,
            expected_result: expected,
        });
    }

    {
        let points: Vec<_> = (0..8)
            .map(|_| G1Projective::rand(&mut rng).into_affine())
            .collect();
        let scalars: Vec<_> = (0..8).map(|_| Fr::rand(&mut rng)).collect();
        let expected: G1Projective = points
            .iter()
            .zip(&scalars)
            .map(|(p, s)| G1Projective::from(*p) * s)
            .sum();

        tests.push(TestVector {
            name: "Eight Point MSM (Power of 2)".to_string(),
            num_points: 8,
            points,
            scalars,
            expected_result: expected,
        });
    }

    {
        let points: Vec<_> = (0..16)
            .map(|_| G1Projective::rand(&mut rng).into_affine())
            .collect();
        let scalars: Vec<_> = (0..16).map(|_| Fr::rand(&mut rng)).collect();
        let expected: G1Projective = points
            .iter()
            .zip(&scalars)
            .map(|(p, s)| G1Projective::from(*p) * s)
            .sum();

        tests.push(TestVector {
            name: "16-Point MSM".to_string(),
            num_points: 16,
            points,
            scalars,
            expected_result: expected,
        });
    }

    {
        let points: Vec<_> = (0..32)
            .map(|_| G1Projective::rand(&mut rng).into_affine())
            .collect();
        let scalars: Vec<_> = (0..32).map(|_| Fr::rand(&mut rng)).collect();
        let expected: G1Projective = points
            .iter()
            .zip(&scalars)
            .map(|(p, s)| G1Projective::from(*p) * s)
            .sum();

        tests.push(TestVector {
            name: "32-Point MSM (Typical Batch)".to_string(),
            num_points: 32,
            points,
            scalars,
            expected_result: expected,
        });
    }

    {
        let points: Vec<_> = (0..256)
            .map(|_| G1Projective::rand(&mut rng).into_affine())
            .collect();
        let scalars: Vec<_> = (0..256).map(|_| Fr::rand(&mut rng)).collect();
        let expected: G1Projective = points
            .iter()
            .zip(&scalars)
            .map(|(p, s)| G1Projective::from(*p) * s)
            .sum();

        tests.push(TestVector {
            name: "256-Point MSM (Maximum Batch)".to_string(),
            num_points: 256,
            points,
            scalars,
            expected_result: expected,
        });
    }

    {
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::from(0u64);
        let expected = G1Projective::from(point) * scalar;

        tests.push(TestVector {
            name: "Zero Scalar Edge Case".to_string(),
            num_points: 1,
            points: vec![point],
            scalars: vec![scalar],
            expected_result: expected,
        });
    }

    {
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::from(1u64);
        let expected = G1Projective::from(point) * scalar;

        tests.push(TestVector {
            name: "One Scalar Edge Case".to_string(),
            num_points: 1,
            points: vec![point],
            scalars: vec![scalar],
            expected_result: expected,
        });
    }

    tests
}

#[test]
fn test_hw_verification_suite() {
    println!("\n=== Hardware MSM Verification Test Suite ===\n");

    let tests = generate_test_vectors();

    let mut passed = 0;
    let mut failed = 0;

    for test in &tests {
        if test.verify_software() {
            passed += 1;
        } else {
            failed += 1;
        }
    }
   
    println!("\n Results ");
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    println!("Total:  {}", passed + failed);

    let mut sv_file = File::create("test_vectors.sv")
        .expect("Failed to create test_vectors.sv");

    writeln!(sv_file, "// Auto-generated test vectors for hardware simulation\n").ok();
    for test in &tests {
        writeln!(sv_file, "{}\n", test.to_systemverilog()).ok();
    }

    println!("\n✓ Generated test_vectors.sv for hardware simulation");

    assert_eq!(failed, 0, "All hardware verification tests must pass");
}
