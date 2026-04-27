use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::Group;
use ark_ff::{BigInteger, PrimeField, Zero};

pub struct PippengerMSM;

impl PippengerMSM {
    pub fn new() -> Self {
        PippengerMSM
    }

    /// Optimized MSM using Pippenger's bucket algorithm with window decomposition.
    /// - Window size: 5 bits (32 buckets per window)
    /// - Process from high to low window to minimize doubling operations
    /// - Bucket reduction via running sum formula: W_j = Σ_{ℓ=1}^{2^w-1} ℓ·B_{ℓ,j}
    pub fn compute(&self, points: &[G1Affine], scalars: &[Fr]) -> G1Projective {
        if points.is_empty() {
            return G1Projective::zero();
        }

        assert_eq!(points.len(), scalars.len());

        const WINDOW_SIZE: usize = 5;
        const BUCKET_COUNT: usize = 1 << WINDOW_SIZE;

        let scalar_bits = Fr::MODULUS_BIT_SIZE as usize;
        let num_windows = (scalar_bits + WINDOW_SIZE - 1) / WINDOW_SIZE;

        let mut result = G1Projective::zero();

        for window_idx in (0..num_windows).rev() {
            for _ in 0..WINDOW_SIZE {
                result.double_in_place();
            }

            let mut buckets = vec![G1Projective::zero(); BUCKET_COUNT];

            for (point, scalar) in points.iter().zip(scalars.iter()) {
                let bits = scalar.into_bigint();
                let window_start = window_idx * WINDOW_SIZE;
                let mut bucket_idx = 0usize;

                for i in 0..WINDOW_SIZE {
                    let bit = bits.get_bit(window_start + i);
                    if bit {
                        bucket_idx |= 1 << i;
                    }
                }

                if bucket_idx > 0 {
                    buckets[bucket_idx] += G1Projective::from(*point);
                }
            }

            let mut bucket_sum = G1Projective::zero();
            let mut window_result = G1Projective::zero();

            for idx in (1..BUCKET_COUNT).rev() {
                bucket_sum += &buckets[idx];
                window_result += &bucket_sum;
            }

            result += window_result;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ec::CurveGroup;
    use ark_ff::UniformRand;
    use rand::rngs::OsRng;

    #[test]
    fn pippenger_single_point() {
        let mut rng = OsRng;
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::rand(&mut rng);

        let msm = PippengerMSM::new();
        let result = msm.compute(&vec![point], &vec![scalar]);
        let expected = G1Projective::from(point) * scalar;

        assert_eq!(result, expected);
    }

    #[test]
    fn pippenger_multiple_points() {
        let mut rng = OsRng;
        let n = 32;

        let points: Vec<_> = (0..n)
            .map(|_| G1Projective::rand(&mut rng).into_affine())
            .collect();
        let scalars: Vec<_> = (0..n).map(|_| Fr::rand(&mut rng)).collect();

        let msm = PippengerMSM::new();
        let result = msm.compute(&points, &scalars);

        let expected: G1Projective = points
            .iter()
            .zip(&scalars)
            .map(|(p, s)| G1Projective::from(*p) * s)
            .sum();

        assert_eq!(result, expected);
    }

    #[test]
    fn pippenger_zero_scalar() {
        let mut rng = OsRng;
        let point = G1Projective::rand(&mut rng).into_affine();
        let scalar = Fr::from(0u64);

        let msm = PippengerMSM::new();
        let result = msm.compute(&vec![point], &vec![scalar]);

        assert_eq!(result, G1Projective::zero());
    }
}
