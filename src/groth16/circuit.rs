use ark_bn254::{Fr, G1Affine};
use ark_r1cs_std::prelude::*;
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

/// Groth16 circuit: proves knowledge of scalars such that Σ scalar_i = sum
///
/// Public inputs: [sum]
/// Private witness: [scalars]
pub struct MSMCircuit {
    pub points: Vec<G1Affine>,
    pub result: G1Affine,
    pub scalars: Vec<Fr>,
}

impl MSMCircuit {
    /// Create an empty circuit for setup (n points/scalars, all zeros)
    pub fn empty(n: usize) -> Self {
        MSMCircuit {
            points: vec![G1Affine::identity(); n],
            result: G1Affine::identity(),
            scalars: vec![Fr::from(0u32); n],
        }
    }
}

impl ConstraintSynthesizer<Fr> for MSMCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // Compute the actual sum of scalars (will be public input)
        let sum: Fr = self.scalars.iter().sum();

        // Allocate sum as public input
        let sum_var = FpVar::new_input(cs.clone(), || Ok(sum))?;

        // Allocate scalars as private witnesses
        let scalar_vars: Vec<_> = self
            .scalars
            .iter()
            .map(|s| FpVar::new_witness(cs.clone(), || Ok(*s)))
            .collect::<Result<_, _>>()?;

        // Constraint: Σ scalar_i == sum
        let mut computed_sum = FpVar::Constant(Fr::from(0u32));
        for scalar_var in scalar_vars.iter() {
            computed_sum += scalar_var;
        }

        computed_sum.enforce_equal(&sum_var)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::UniformRand;
    use ark_groth16::Groth16;
    use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
    use ark_bn254::{Bn254, G1Projective};
    use rand::rngs::OsRng;

    #[test]
    fn test_msmcircuit_satisfiable() {
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

        // Compute sum of scalars for public input
        let scalar_sum: Fr = scalars.iter().sum();

        let circuit = MSMCircuit {
            points: points.clone(),
            result,
            scalars: scalars.clone(),
        };

        let (pk, vk) = Groth16::<Bn254>::setup(
            MSMCircuit::empty(n),
            &mut rng,
        )
        .expect("setup failed");

        let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng)
            .expect("prove failed");

        let public_inputs = vec![scalar_sum];
        let verified = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof)
            .expect("verify failed");

        assert!(verified, "proof verification failed");
    }
}
