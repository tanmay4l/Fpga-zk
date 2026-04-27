use ark_bls12_381::{Fr, G1Affine, G1Projective};
use crate::pippenger::PippengerMSM;

pub trait MSMAccelerator: Send + Sync {
    fn compute(&self, points: &[G1Affine], scalars: &[Fr]) -> G1Projective;
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
}

impl MSMAccelerator for PippengerMSM {
    fn compute(&self, points: &[G1Affine], scalars: &[Fr]) -> G1Projective {
        PippengerMSM::compute(self, points, scalars)
    }

    fn name(&self) -> &'static str {
        "Pippenger MSM"
    }

    fn is_available(&self) -> bool {
        true
    }
}

pub struct HardwareMSM;

impl MSMAccelerator for HardwareMSM {
    fn compute(&self, points: &[G1Affine], scalars: &[Fr]) -> G1Projective {
        PippengerMSM::new().compute(points, scalars)
    }

    fn name(&self) -> &'static str {
        "Hardware (with Pippenger fallback)"
    }

    fn is_available(&self) -> bool {
        false
    }
}

pub fn create_accelerator(prefer_hardware: bool) -> Box<dyn MSMAccelerator> {
    if prefer_hardware {
        Box::new(HardwareMSM)
    } else {
        Box::new(PippengerMSM::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accelerator_factory() {
        let accel = create_accelerator(true);
        assert!(!accel.name().is_empty());
    }
}
