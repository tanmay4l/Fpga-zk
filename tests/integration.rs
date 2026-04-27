use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::{UniformRand, Zero};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fpga_zk::NaiveMSM;
use rand::rngs::OsRng;

#[test]
fn msm_single_point() {
    let mut rng = OsRng;
    let point = G1Projective::rand(&mut rng).into_affine();
    let scalar = Fr::rand(&mut rng);

    let accel = NaiveMSM::new();
    let result = accel.compute(&vec![point], &vec![scalar]);

    let expected = G1Projective::from(point) * scalar;
    assert_eq!(result, expected);
}

#[test]
fn msm_multiple_points() {
    let mut rng = OsRng;
    let n = 16;

    let points: Vec<_> = (0..n)
        .map(|_| G1Projective::rand(&mut rng).into_affine())
        .collect();
    let scalars: Vec<_> = (0..n).map(|_| Fr::rand(&mut rng)).collect();

    let accel = NaiveMSM::new();
    let result = accel.compute(&points, &scalars);

    let expected: G1Projective = points
        .iter()
        .zip(&scalars)
        .map(|(p, s)| G1Projective::from(*p) * s)
        .sum();

    assert_eq!(result, expected);
}

#[test]
fn msm_serialization_roundtrip() {
    let mut rng = OsRng;
    let n = 4;

    let points: Vec<_> = (0..n)
        .map(|_| G1Projective::rand(&mut rng).into_affine())
        .collect();
    let scalars: Vec<_> = (0..n).map(|_| Fr::rand(&mut rng)).collect();

    let accel = NaiveMSM::new();
    let result = accel.compute(&points, &scalars);
    let result_affine = result.into_affine();

    let mut serialized = Vec::new();
    result_affine.serialize_compressed(&mut serialized).unwrap();

    let deserialized = G1Affine::deserialize_compressed(serialized.as_slice()).unwrap();
    assert_eq!(result_affine, deserialized);
}

#[test]
fn msm_zero_scalar() {
    let mut rng = OsRng;
    let point = G1Projective::rand(&mut rng).into_affine();
    let scalar = Fr::from(0u64);

    let accel = NaiveMSM::new();
    let result = accel.compute(&vec![point], &vec![scalar]);

    assert_eq!(result, G1Projective::zero());
}

#[test]
fn msm_one_scalar() {
    let mut rng = OsRng;
    let point = G1Projective::rand(&mut rng).into_affine();
    let scalar = Fr::from(1u64);

    let accel = NaiveMSM::new();
    let result = accel.compute(&vec![point], &vec![scalar]);

    assert_eq!(result.into_affine(), point);
}
