use serde::{Serialize, Deserialize};

use crate::{Hasher, Scalar, AsBase64};
use crate::curve::CurveElem;
use crate::elgamal::{CryptoContext, Ciphertext};
use std::fmt::Display;
use serde::export::Formatter;

const KNOW_PLAINTEXT_TAG: &'static str = "KNOW_PLAINTEXT";

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PrfKnowPlaintext {
    pub g: CurveElem,
    pub ct: Ciphertext,
    blinded_g: CurveElem,
    r: Scalar,
}

impl Display for PrfKnowPlaintext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}:{}", self.g.as_base64(), self.ct.to_string(),
               self.blinded_g.as_base64(), self.r.as_base64())
    }
}

impl PrfKnowPlaintext {
    fn challenge(g: &CurveElem, ct: &Ciphertext, blinded_g: &CurveElem) -> Scalar {
        Hasher::sha_256()
            .and_update(&g.as_bytes())
            .and_update(&ct.c1.as_bytes())
            .and_update(&ct.c2.as_bytes())
            .and_update(&blinded_g.as_bytes())
            .and_update(KNOW_PLAINTEXT_TAG.as_bytes())
            .finish_scalar()
    }

    pub fn new(ctx: &CryptoContext, ct: Ciphertext, r: Scalar) -> Self {
        // Choose random commitment
        let g = ctx.generator();
        let z = ctx.random_scalar();
        let blinded_g = g.scaled(&z);
        // Calculate the challenge
        let c = Self::challenge(&g, &ct, &blinded_g);
        let r = Scalar(z.0 + c.0 * r.0);

        Self { g, ct, blinded_g, r }
    }

    pub fn verify(&self) -> bool {
        let c = Self::challenge(&self.g, &self.ct, &self.blinded_g);
        self.g.scaled(&self.r) == &self.blinded_g + &self.ct.c1.scaled(&c)
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PrfEqDlogs {
    pub result1: CurveElem,
    pub base1: CurveElem,
    pub result2: CurveElem,
    pub base2: CurveElem,
    blinded_base1: CurveElem,
    blinded_base2: CurveElem,
    r: Scalar,
}

impl Display for PrfEqDlogs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}:{}:{}:{}:{}", self.result1.as_base64(), self.base1.as_base64(),
               self.result2.as_base64(), self.base2.as_base64(), self.blinded_base1.as_base64(),
               self.blinded_base2.as_base64(), self.r.as_base64())
    }
}

const EQ_DLOGS_TAG: &'static str = "EQ_DLOGS";

impl PrfEqDlogs {
    fn challenge(f: &CurveElem,
                 h: &CurveElem,
                 v: &CurveElem,
                 w: &CurveElem,
                 a: &CurveElem,
                 b: &CurveElem) -> Scalar {
        Hasher::sha_256()
            .and_update(&f.as_bytes())
            .and_update(&h.as_bytes())
            .and_update(&v.as_bytes())
            .and_update(&w.as_bytes())
            .and_update(&a.as_bytes())
            .and_update(&b.as_bytes())
            .and_update(EQ_DLOGS_TAG.as_bytes())
            .finish_scalar()
    }

    /// Prove that v = f^x and w = h^x, i.e. that dlog_f v = dlog_h w for a secret x
    pub fn new(ctx: &CryptoContext,
               base1: &CurveElem,
               base2: &CurveElem,
               result1: &CurveElem,
               result2: &CurveElem,
               power: &Scalar) -> Self {
        let z = ctx.random_scalar();
        let blinded_base1 = base1.scaled(&z);
        let blinded_base2 = base2.scaled(&z);
        let c = Self::challenge(&base1, &base2, &result1, &result2, &blinded_base1, &blinded_base2);
        let r = Scalar(z.0 + c.0 * power.0);
        Self {
            result1: result1.clone(),
            base1: base1.clone(),
            result2: result2.clone(),
            base2: base2.clone(),
            blinded_base1,
            blinded_base2,
            r
        }
    }

    pub fn verify(&self) -> bool {
        let c = Self::challenge(&self.base1, &self.base2, &self.result1, &self.result2, &self.blinded_base1, &self.blinded_base2);
        self.base1.scaled(&self.r) == &self.blinded_base1 + &self.result1.scaled(&c)
            && self.base2.scaled(&self.r) == &self.blinded_base2 + &self.result2.scaled(&c)
    }
}

const DECRYPTION_TAG: &'static str = "DECRYPTION";

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PrfDecryption {
    pub g: CurveElem,
    pub ct: Ciphertext,
    pub public_key: CurveElem,
    pub dec_factor: CurveElem,
    blinded_g: CurveElem,
    blinded_c1: CurveElem,
    r: Scalar,
}

impl PrfDecryption {
    fn challenge(g: &CurveElem, ct: &Ciphertext, dec_factor: &CurveElem, public_key: &CurveElem) -> Scalar {
        Hasher::sha_256()
            .and_update(&g.as_bytes())
            .and_update(&ct.c1.as_bytes())
            .and_update(&ct.c2.as_bytes())
            .and_update(&dec_factor.as_bytes())
            .and_update(&public_key.as_bytes())
            .and_update(DECRYPTION_TAG.as_bytes())
            .finish_scalar()
    }

    pub fn new(ctx: &CryptoContext, ct: Ciphertext, dec_factor: CurveElem, secret: Scalar, public_key: CurveElem) -> Self {
        let g = ctx.generator();

        let z = ctx.random_scalar();
        let blinded_g = g.scaled(&z);
        let blinded_c1 = ct.c1.scaled(&z);

        let c = Self::challenge(&g, &ct, &dec_factor, &public_key);

        let r = Scalar(z.0 + c.0 * secret.0);

        Self { g, ct, public_key, dec_factor, blinded_g, blinded_c1, r }
    }

    pub fn verify(&self) -> bool {
        let c = Self::challenge(&self.g, &self.ct, &self.dec_factor, &self.public_key);
        self.g.scaled(&self.r) == &self.blinded_g + &self.public_key.scaled(&c)
            && self.ct.c1.scaled(&self.r) == &self.blinded_c1 + &self.dec_factor.scaled(&c)
    }
}

#[cfg(test)]
mod tests {
    use crate::elgamal::{CryptoContext, PublicKey};
    use crate::zkp::{PrfEqDlogs, PrfDecryption, PrfKnowPlaintext};
    use crate::Scalar;
    use crate::scalar::DalekScalar;

    #[test]
    fn test_exp_sum() {
        let ctx = CryptoContext::new().unwrap();
        let a = ctx.random_scalar();
        let b = ctx.random_scalar();
        let r = Scalar(a.0 + b.0);

        let x = ctx.g_to(&r);
        let y = ctx.g_to(&a) + ctx.g_to(&b);
        assert_eq!(x, y);
    }

    #[test]
    fn test_prf_know_plaintext_complete() {
        let ctx = CryptoContext::new().unwrap();
        let x = ctx.random_scalar();
        let pk = PublicKey::new(ctx.g_to(&x));

        let m = ctx.random_elem();
        let r = ctx.random_scalar();
        let enc = pk.encrypt(&ctx, &m, &r);

        let proof = PrfKnowPlaintext::new(&ctx, enc, r);
        assert!(proof.verify());
    }

    #[test]
    fn test_prf_know_plaintext_sound() {
        let ctx = CryptoContext::new().unwrap();
        let x = ctx.random_scalar();
        let pk = PublicKey::new(ctx.g_to(&x));

        let m = ctx.random_elem();
        let r = ctx.random_scalar();
        let enc = pk.encrypt(&ctx, &m, &r);

        let mut proof = PrfKnowPlaintext::new(&ctx, enc, r);
        proof.r.0 += &DalekScalar::one();
        assert!(!proof.verify());
    }

    #[test]
    fn test_prf_eq_dlogs_complete() {
        let ctx = CryptoContext::new().unwrap();
        let x1 = ctx.random_scalar();
        let f = ctx.g_to(&x1);
        let x2 = ctx.random_scalar();
        let h = ctx.g_to(&x2);

        let x = ctx.random_scalar();
        let v = f.scaled(&x);
        let w = h.scaled(&x);

        let proof = PrfEqDlogs::new(&ctx, &f, &h, &v, &w, &x);
        assert!(proof.verify());
    }

    #[test]
    fn test_prf_eq_dlogs_sound() {
        let ctx = CryptoContext::new().unwrap();
        let x1 = ctx.random_scalar();
        let f = ctx.g_to(&x1);
        let x2 = ctx.random_scalar();
        let h = ctx.g_to(&x2);

        let x = ctx.random_scalar();
        let v = f.scaled(&x);
        let w = h.scaled(&x);

        let mut proof = PrfEqDlogs::new(&ctx, &f, &h, &v, &w, &x);
        proof.r.0 += &DalekScalar::one();

        assert!(!proof.verify());
    }

    #[test]
    fn test_prf_dec_complete() {
        let ctx = CryptoContext::new().unwrap();
        let x = ctx.random_scalar();
        let pk = PublicKey::new(ctx.g_to(&x));

        let m = ctx.random_elem();
        let r = ctx.random_scalar();
        let enc = pk.encrypt(&ctx, &m, &r);
        let dec = enc.c1.scaled(&x);

        let proof = PrfDecryption::new(&ctx, enc, dec, x, pk.y);
        assert!(proof.verify());
    }

    #[test]
    fn test_prf_dec_sound() {
        let ctx = CryptoContext::new().unwrap();
        let x = ctx.random_scalar();
        let pk = PublicKey::new(ctx.g_to(&x));

        let m = ctx.random_elem();
        let r = ctx.random_scalar();
        let enc = pk.encrypt(&ctx, &m, &r);
        let dec = enc.c1.scaled(&x);

        let mut proof = PrfDecryption::new(&ctx, enc, dec, x, pk.y);
        proof.r.0 += &DalekScalar::one();

        assert!(!proof.verify());
    }
}
