use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::{Mutex, OnceLock};

use loopal_secret_client::{ExposeSecret, SecretString};

use crate::secret_expand::CONFIG_SECRET_ERROR;

#[derive(Default)]
pub(crate) struct SecretProvenance {
    expected: Mutex<Option<Fingerprint>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Fingerprint(u64, u64);

impl SecretProvenance {
    pub(crate) fn establish(&self, seed: &[(String, SecretString)]) -> Result<(), &'static str> {
        let fingerprint = fingerprint(seed);
        let mut expected = self.expected.lock().map_err(|_| CONFIG_SECRET_ERROR)?;
        match *expected {
            Some(value) if value != fingerprint => Err(CONFIG_SECRET_ERROR),
            Some(_) => Ok(()),
            None => {
                *expected = Some(fingerprint);
                Ok(())
            }
        }
    }

    pub(crate) fn reset(&self) -> Result<(), &'static str> {
        *self.expected.lock().map_err(|_| CONFIG_SECRET_ERROR)? = None;
        Ok(())
    }
}

fn fingerprint(seed: &[(String, SecretString)]) -> Fingerprint {
    static KEYS: OnceLock<(
        std::collections::hash_map::RandomState,
        std::collections::hash_map::RandomState,
    )> = OnceLock::new();
    let keys = KEYS.get_or_init(Default::default);
    Fingerprint(hash_seed(&keys.0, seed), hash_seed(&keys.1, seed))
}

fn hash_seed(
    key: &std::collections::hash_map::RandomState,
    seed: &[(String, SecretString)],
) -> u64 {
    let mut hasher = key.build_hasher();
    seed.len().hash(&mut hasher);
    for (name, value) in seed {
        name.hash(&mut hasher);
        value.expose_secret().hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
#[path = "secret_provenance_tests.rs"]
mod tests;
