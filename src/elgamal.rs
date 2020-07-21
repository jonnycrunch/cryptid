use std::fmt::{Formatter, Display};
use std::sync::{Mutex, Arc};

use ring::rand::SecureRandom;
use serde::{Serialize, Deserialize};

use crate::{curve, CryptoError, Hasher};
use crate::Scalar;
use std::hash::Hash;
use crate::util::{AsBase64, SCALAR_MAX_BYTES};
use std::convert::{TryFrom, TryInto};
use crate::threshold::EncodingError;

#[derive(Copy, Clone)]
pub struct KeyPair {
    pub pk: PublicKey,
    pub x_i: Scalar,
    pub y_i: CurveElem,
}

impl KeyPair {
    fn new(ctx: &mut CryptoContext) -> Result<Self, CryptoError> {
        let x_i = ctx.random_power()?;
        let y_i = ctx.g_to(&x_i);
        let pk = PublicKey::new(y_i);
        Ok(Self { pk, x_i, y_i })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct PublicKey {
    y: CurveElem,
}

impl PublicKey {
    pub fn new(value: CurveElem) -> Self {
        Self { y: value }
    }

    pub fn encrypt(&self, ctx: &CryptoContext, m: &CurveElem, r: &Scalar) -> Ciphertext {
        let c1 = ctx.g_to(r);
        let c2 = m + &self.y.scaled(r);
        Ciphertext { c1, c2 }
    }

    pub fn encrypt_auth(&self, ctx: &CryptoContext, m: &CurveElem, r: &Scalar) -> AuthCiphertext {
        let ct = self.encrypt(ctx, m, r);
        AuthCiphertext::new(&ct, m)
    }

    pub fn rerand(self, ctx: &CryptoContext, ct: &Ciphertext, r: &Scalar) -> Ciphertext {
        let c1 = &ct.c1 + &ctx.g_to(r);
        let c2 = &ct.c2 + &self.y.scaled(r);
        Ciphertext { c1, c2 }
    }
}

impl AsBase64 for PublicKey {
    type Error = CryptoError;

    fn as_base64(&self) -> String {
        self.y.as_base64()
    }

    fn try_from_base64(encoded: &str) -> Result<Self, Self::Error> {
        Ok(Self {
            y: CurveElem::try_from_base64(encoded)?
        })
    }
}

impl Hash for PublicKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_base64().hash(state);
    }
}

impl Display for PublicKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.y.as_base64())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ciphertext {
    pub c1: CurveElem,
    pub c2: CurveElem,
}

impl Ciphertext {
    pub fn add(&self, rhs: &Self) -> Self {
        Ciphertext {
            c1: &self.c1 + &rhs.c1,
            c2: &self.c2 + &rhs.c2,
        }
    }

    pub fn decrypt(&self, secret_key: &Scalar) -> CurveElem {
        &self.c2 - &(self.c1.scaled(secret_key))
    }
}

impl ToString for Ciphertext {
    fn to_string(&self) -> String {
        format!("{}:{}", self.c1.as_base64(), self.c2.as_base64())
    }
}

impl TryFrom<String> for Ciphertext {
    type Error = EncodingError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut elems = Vec::new();
        for encoded in value.split(":") {
            let elem = CurveElem::try_from_base64(encoded).map_err(|_| EncodingError::CurveElem)?;
            elems.push(elem);
        }
        if elems.len() != 2 {
            Err(EncodingError::Length)
        } else {
            Ok(Self {
                c1: elems[0],
                c2: elems[1],
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCiphertext {
    pub contents: Ciphertext,
    hash: Vec<u8>,
}

impl AuthCiphertext {
    fn new(ct: &Ciphertext, plaintext: &CurveElem) -> Self {
        let hash = Hasher::sha_512()
            .and_update(&plaintext.as_bytes())
            .finish().as_ref().to_vec();
        Self { contents: ct.clone(), hash }
    }

    pub fn verify(&self, plaintext: &CurveElem) -> bool {
        let hash = Hasher::sha_512()
            .and_update(&plaintext.as_bytes())
            .finish().as_ref().to_vec();

        self.hash == hash
    }

    pub fn decrypt(&self, secret_key: &Scalar) -> Option<CurveElem> {
        let plaintext = self.contents.decrypt(secret_key);
        if self.verify(&plaintext) {
            Some(plaintext)
        } else {
            None
        }
    }
}

impl Display for AuthCiphertext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})[{}]",
               self.contents.c1.as_base64(),
               self.contents.c2.as_base64(),
               base64::encode(&self.hash))
    }
}

pub type CurveElem = curve::CurveElem;

#[derive(Debug)]
pub struct CryptoContext {
    rng: Arc<Mutex<ring::rand::SystemRandom>>,
    g: CurveElem,
}

impl Clone for CryptoContext {
    fn clone(&self) -> Self {
        let rng = self.rng.clone();
        let g = self.g.clone();
        Self { rng, g }
    }
}

impl CryptoContext {
    pub fn new() -> Self {
        let rng = Arc::new(Mutex::new(ring::rand::SystemRandom::new()));
        let g = CurveElem::generator();
        Self {
            rng,
            g
        }
    }

    pub fn rng(&self) -> Arc<Mutex<ring::rand::SystemRandom>> {
        self.rng.clone()
    }

    pub fn generator(&self) -> CurveElem {
        self.g.clone()
    }

    pub fn gen_elgamal_key_pair(&mut self) -> Result<KeyPair, CryptoError> {
        KeyPair::new(self)
    }

    pub fn random_power(&mut self) -> Result<Scalar, CryptoError> {
        let rng = self.rng.lock().unwrap();
        let mut buf = [0; 32];
        rng.fill(&mut buf)
            .map_err(|e| CryptoError::Unspecified(e))?;
        Ok(buf.into())
    }

    pub fn random_elem(&mut self) -> Result<CurveElem, CryptoError> {
        let rng = self.rng.lock().unwrap();

        let mut buf = [0; SCALAR_MAX_BYTES];
        rng.fill(&mut buf)
            .map_err(|e| CryptoError::Unspecified(e))?;

        let mut final_buf = [0u8; 32];
        for (i, byte) in buf.iter().enumerate() {
            final_buf[i] = *byte;
        }

        let s: Scalar = final_buf.into();
        s.try_into()
    }

    pub fn g_to(&self, power: &Scalar) -> CurveElem {
        self.g.scaled(power)
    }
}

#[cfg(test)]
mod test {
    use crate::elgamal::{CryptoContext, PublicKey, Ciphertext, AuthCiphertext};
    use crate::util::AsBase64;
    use std::convert::TryFrom;

    #[test]
    fn test_pubkey_serde() {
        let mut ctx = CryptoContext::new();
        let x = ctx.random_power().unwrap();
        let y = PublicKey::new(ctx.g_to(&x).into());

        let encoded = y.as_base64();
        let decoded = PublicKey::try_from_base64(encoded.as_str()).unwrap();
        assert_eq!(y, decoded);
    }

    #[test]
    fn test_ciphertext_serde() {
        let mut ctx = CryptoContext::new();
        let x = ctx.random_power().unwrap();
        let y = PublicKey::new(ctx.g_to(&x).into());

        let r = ctx.random_power().unwrap();
        let m = ctx.g_to(&r);

        let r = ctx.random_power().unwrap();

        let ct = y.encrypt(&ctx, &m.into(), &r);

        assert_eq!(ct, Ciphertext::try_from(ct.to_string()).unwrap());
    }

    #[test]
    fn test_homomorphism() {
        let mut ctx = CryptoContext::new();
        let x = ctx.random_power().unwrap();
        let y = PublicKey::new(ctx.g_to(&x).into());

        // Construct two messages
        let r1 = ctx.random_power().unwrap();
        let r2 = ctx.random_power().unwrap();
        let m1 = ctx.g_to(&r1);
        let m2 = ctx.g_to(&r2);

        let r1 = ctx.random_power().unwrap();
        let r2 = ctx.random_power().unwrap();

        // Encrypt the messages
        let ct1 = y.encrypt(&ctx, &m1.into(), &r1);
        let ct2 = y.encrypt(&ctx, &m2.into(), &r2);

        // Compare the added encryption to the added messages
        let prod = ct1.add(&ct2);
        // let decryption = &prod.c2 - &(prod.c1.scaled(&x));
        let decryption = prod.decrypt(&x);

        let combined = &m1 + &m2;

        assert_eq!(combined, decryption);
    }

    #[test]
    fn test_authtag() {
        let mut ctx = CryptoContext::new();
        let x = ctx.random_power().unwrap();
        let y = PublicKey::new(ctx.g_to(&x).into());

        let r = ctx.random_power().unwrap();
        let m = ctx.g_to(&r);
        let m_r = ctx.random_power().unwrap();
        let ct = y.encrypt_auth(&ctx, &m, &m_r);

        assert_eq!(ct.decrypt(&x).unwrap(), m);
    }

    #[test]
    fn test_authtag_fail() {
        let mut ctx = CryptoContext::new();
        let x = ctx.random_power().unwrap();
        let y = PublicKey::new(ctx.g_to(&x).into());

        let r = ctx.random_power().unwrap();
        let m = ctx.g_to(&r);
        let m_r = ctx.random_power().unwrap();
        let ct = y.encrypt_auth(&ctx, &m, &m_r);

        let r = ctx.random_power().unwrap();
        let m_dash = ctx.g_to(&r);
        let ct_modified = Ciphertext {
            c1: ct.contents.c1,
            c2: ct.contents.c2 + m_dash,
        };
        let auth_modified = AuthCiphertext {
            contents: ct_modified.clone(),
            hash: ct.hash.clone(),
        };

        assert!(!auth_modified.verify(&(m + m_dash)));
        assert_eq!(ct_modified.decrypt(&x), m + m_dash);
    }
}