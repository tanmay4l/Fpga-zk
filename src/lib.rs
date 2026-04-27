pub mod hw_accel;
pub mod msm;
pub mod pippenger;
pub mod groth16;

pub use msm::NaiveMSM;
pub use hw_accel::{MSMAccelerator, create_accelerator, HardwareMSM};
pub use pippenger::PippengerMSM;

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::{Fr, G1Projective};
    use ark_ec::CurveGroup;
    use ark_ff::Zero;
    use rand::rngs::OsRng;

    #[test]
    fn test_msm_single_point() {
        use ark_ff::UniformRand;
        let mut rng = OsRng;
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::rand(&mut rng);

        let accel = NaiveMSM::new();
        let result = accel.compute(&vec![point], &vec![scalar]).into_affine();
        let expected = (G1Projective::from(point) * scalar).into_affine();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_msm_multiple_points() {
        use ark_ff::UniformRand;
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
    fn test_msm_zero_scalar() {
        use ark_ff::UniformRand;
        let mut rng = OsRng;
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::from(0u64);

        let accel = NaiveMSM::new();
        let result = accel.compute(&vec![point], &vec![scalar]);

        assert_eq!(result, G1Projective::zero());
    }
}
