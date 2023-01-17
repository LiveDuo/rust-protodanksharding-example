use ff::Field;
mod arkworks;

pub mod test_utils;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

mod kzg;
mod polynomial;
mod roots_of_unity;

pub type G1Point = blstrs::G1Affine;
pub type G2Point = blstrs::G2Affine;
pub type Scalar = blstrs::Scalar;
pub type KZGCommitment = G1Point;

// The number of bytes needed to represent a scalar
pub const SCALAR_SERIALISED_SIZE: usize = 32;
// The number of bytes needed to represent a compressed G1 point
pub const G1_POINT_SERIALISED_SIZE: usize = 48;

pub(crate) type G1Projective = blstrs::G1Projective;

pub use kzg::{
    aggregated_proof::AggregatedKZG,
    proof::{KZGProof, KZGWitness},
    srs::PublicParameters,
};
pub use polynomial::Polynomial;
pub use roots_of_unity::RootsOfUnity;

pub(crate) fn batch_inverse(elements: &mut [Scalar]) {
    batch_inversion(elements)
}

// Invert a field element, returning 0 if the element
// is not invertible
pub(crate) fn inverse(x: Scalar) -> Scalar {
    x.invert().unwrap_or(Scalar::zero())
}

// A multi-scalar multiplication
pub fn g1_lincomb(points: &[G1Point], scalars: &[Scalar]) -> G1Point {
    // TODO: Spec says we should panic, but as a lib its better to return result
    assert_eq!(points.len(), scalars.len());

    // TODO: Blst library needs projective points, so we will clone and convert here
    #[cfg(feature = "rayon")]
    let points_iter = points.into_par_iter();
    #[cfg(not(feature = "rayon"))]
    let points_iter = points.into_iter();

    let points: Vec<_> = points_iter
        .map(|point| blstrs::G1Projective::from(point))
        .collect();

    // TODO: the internal lib seems to be converting back to Affine, so we need to
    // TODO create our own version of this function
    blstrs::G1Projective::multi_exp(&points, scalars).into()
}

// Taken from arkworks codebase
// Given a vector of field elements {v_i}, compute the vector {coeff * v_i^(-1)}
#[cfg(feature = "rayon")]
pub fn batch_inversion(v: &mut [Scalar]) {
    // Divide the vector v evenly between all available cores
    let min_elements_per_thread = 1;
    let num_cpus_available = rayon::current_num_threads();
    let num_elems = v.len();
    let num_elem_per_thread =
        std::cmp::max(num_elems / num_cpus_available, min_elements_per_thread);

    // Batch invert in parallel, without copying the vector
    v.par_chunks_mut(num_elem_per_thread).for_each(|mut chunk| {
        serial_batch_inversion(&mut chunk);
    });
}
#[cfg(not(feature = "rayon"))]
pub fn batch_inversion(v: &mut [Scalar]) {
    serial_batch_inversion(v);
}

/// Given a vector of field elements {v_i}, compute the vector {coeff * v_i^(-1)}
/// This method is explicitly single core.
fn serial_batch_inversion(v: &mut [Scalar]) {
    use std::ops::MulAssign;

    // Montgomery’s Trick and Fast Implementation of Masked AES
    // Genelle, Prouff and Quisquater
    // Section 3.2
    // but with an optimization to multiply every element in the returned vector by coeff

    // First pass: compute [a, ab, abc, ...]
    let mut prod = Vec::with_capacity(v.len());
    let mut tmp = Scalar::one();
    for f in v.iter().filter(|f| !f.is_zero_vartime()) {
        tmp.mul_assign(f);
        prod.push(tmp);
    }

    // Invert `tmp`.
    tmp = tmp.invert().unwrap(); // Guaranteed to be nonzero.

    // Second pass: iterate backwards to compute inverses
    for (f, s) in v
        .iter_mut()
        // Backwards
        .rev()
        // Ignore normalized elements
        .filter(|f| !f.is_zero_vartime())
        // Backwards, skip last element, fill in one for last term.
        .zip(prod.into_iter().rev().skip(1).chain(Some(Scalar::one())))
    {
        // tmp := tmp * f; f := tmp * s = 1/f
        let new_tmp = tmp * *f;
        *f = tmp * &s;
        tmp = new_tmp;
    }
}
