use crate::{domain::Domain, polynomial::Polynomial};

// The key that is used to commit to polynomials in monomial form
//
/// Group elements of the form `{ \tau^i G }`
///  Where:
/// - `i` ranges from 0 to `degree`.
/// - `G` is some generator of the group
pub struct CommitKey { inner: Vec<blstrs::G1Affine>, }

impl CommitKey {
    pub fn new(points: Vec<blstrs::G1Affine>) -> CommitKey {
        assert!(
            !points.is_empty(),
            "cannot initialize `CommitKey` with no points"
        );
        CommitKey { inner: points }
    }

    // Note: There is no commit method for CommitKey in monomial basis as this is not used
    pub fn into_lagrange(self, domain: &Domain) -> CommitKeyLagrange {
        CommitKeyLagrange { inner: domain.ifft_g1(self.inner) }
    }
}

// The key that is used to commit to polynomials in lagrange form
//
/// Group elements of the form `{ \L_i(\tau) * G }`
/// Where :
/// - `i` ranges from 0 to `degree`
/// -  L_i is the i'th lagrange polynomial
/// - `G` is some generator of the group
pub struct CommitKeyLagrange { inner: Vec<blstrs::G1Affine>, }

impl CommitKeyLagrange {
    pub fn new(points: Vec<blstrs::G1Affine>) -> CommitKeyLagrange {
        assert!(points.len() > 1);
        CommitKeyLagrange { inner: points }
    }

    /// Commit to `polynomial` in lagrange form
    pub fn commit(&self, polynomial: &Polynomial) -> blstrs::G1Affine {
        g1_lincomb(&self.inner, &polynomial.evaluations)
    }

    /// Returns the maximum degree polynomial that one can commit to
    /// Since we are in lagrange basis, it is the number of points minus one
    ///
    /// Example: Given f(z) = z^2 , the number of evaluation points needed
    /// to represent f(z) is 3, but its degree is 2
    pub fn max_degree(&self) -> usize {
        self.inner.len() - 1
    }
}

// A multi-scalar multiplication
pub fn g1_lincomb(points: &[blstrs::G1Affine], scalars: &[blstrs::Scalar]) -> blstrs::G1Affine {
    // TODO: We could use arkworks here and use their parallelized multi-exp instead

    // TODO: Spec says we should panic, but as a lib its better to return result
    assert_eq!(points.len(), scalars.len());

    let points_iter = points.into_iter();

    let points: Vec<_> = points_iter
        .map(|point| blstrs::G1Projective::from(point))
        .collect();

    // blst does not use multiple threads
    // TODO: the internal lib seems to be converting back to Affine
    blstrs::G1Projective::multi_exp(&points, scalars).into()
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use group::prime::PrimeCurveAffine;

    use crate::{domain::Domain, commit_key::*};

    fn eval_coeff_poly(poly: &[blstrs::Scalar], input_point: &blstrs::Scalar) -> blstrs::Scalar {
        let mut result = blstrs::Scalar::zero();
        for (index, coeff) in poly.iter().enumerate() {
            result += input_point.pow_vartime(&[index as u64]) * coeff;
        }
        result
    }

    #[test]
    fn transform_srs() {
        let degree = 16;

        let domain = Domain::new(degree);

        // f(x) -- These are the coefficients of the polynomial
        let f_x_coeffs: Vec<_> = (0..degree as u64).into_iter().map(blstrs::Scalar::from).collect();

        // Evaluate f(x) over the domain -- To get the evaluation form of f(x)
        let f_x_evaluations: Vec<_> = domain
            .roots
            .iter()
            .map(|root| eval_coeff_poly(&f_x_coeffs, root))
            .collect();

        let secret = blstrs::Scalar::from(1234567u64);
        let monomial_srs: Vec<blstrs::G1Affine> = (0..degree)
            .map(|index| {
                let secret_exp = secret.pow_vartime(&[index as u64]);
                (blstrs::G1Affine::generator() * secret_exp).into()
            })
            .collect();

        // Commit to f(x) in monomial form
        let expected_commitment = g1_lincomb(&monomial_srs, &f_x_coeffs);

        // Commit to f(x) in lagrange form
        let commit_key = CommitKey { inner: monomial_srs, };
        let lagrange_srs = commit_key.into_lagrange(&domain).inner;
        let got_commitment = g1_lincomb(&lagrange_srs, &f_x_evaluations);

        assert_eq!(expected_commitment, got_commitment)
    }
}
