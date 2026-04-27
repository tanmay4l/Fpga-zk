use ark_bn254::{Fr, G1Affine, Bn254};
use ark_groth16::{Proof, ProvingKey};
use ark_snark::SNARK;

use crate::groth16::circuit::MSMCircuit;


pub fn prove(
    pk: &ProvingKey<Bn254>,
    points: &[G1Affine],
    scalars: &[Fr],
    result: G1Affine,
    rng: &mut (impl rand::RngCore + rand::CryptoRng),
) -> Result<Proof<Bn254>, String> {
    let circuit = MSMCircuit {
        points: points.to_vec(),
        result,
        scalars: scalars.to_vec(),
    };

    ark_groth16::Groth16::<Bn254>::prove(pk, circuit, rng)
        .map_err(|e| format!("Proof generation failed: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::UniformRand;
    use ark_groth16::Groth16;
    use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
    use ark_bn254::G1Projective;
    use rand::rngs::OsRng;

    #[test]
    fn test_prove_and_verify() {
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

        let (pk, vk) = Groth16::<Bn254>::setup(
            MSMCircuit::empty(n),
            &mut rng,
        )
        .expect("setup failed");

        let proof = prove(&pk, &points, &scalars, result, &mut rng)
            .expect("prove failed");

        let verified = Groth16::<Bn254>::verify(&vk, &vec![scalar_sum], &proof)
            .expect("verify failed");

        assert!(verified, "proof verification failed");
    }
}
