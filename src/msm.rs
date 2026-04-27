use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{CurveGroup, Group};
use ark_ff::{BigInteger, PrimeField, Zero};

pub struct MSMAccelerator;

impl MSMAccelerator {
    pub fn new() -> Self {
        MSMAccelerator
    }

    pub fn compute(&self, points: &[G1Affine], scalars: &[Fr]) -> G1Projective {
        if points.is_empty() {
            return G1Projective::zero();
        }

        assert_eq!(points.len(), scalars.len(), "points and scalars length mismatch");

        points
            .iter()
            .zip(scalars.iter())
            .map(|(p, s)| G1Projective::from(*p) * *s)
            .sum()
    }
}

