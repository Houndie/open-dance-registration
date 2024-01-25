use chrono::Utc;
use p256::ecdsa::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

use crate::store::{
    self,
    keys::{Key, Store as KeyStore},
};

pub struct KeyManager<Store: KeyStore> {
    store: Store,
}

impl<Store: KeyStore> KeyManager<Store> {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub async fn rotate_key(&self, clear: bool) -> Result<(), store::Error> {
        let clear_keys = if clear {
            self.store
                .list(vec![])
                .await?
                .into_iter()
                .map(|k| k.id)
                .collect()
        } else {
            vec![]
        };

        let key = Key {
            id: "".to_string(),
            key: SigningKey::random(&mut OsRng),
            created_at: Utc::now(),
        };

        let _ = self.store.insert(key).await?;

        if clear {
            self.store.delete(clear_keys).await?;
        }

        Ok(())
    }

    pub async fn get_signing_key(&self) -> Result<SigningKey, store::Error> {
        Ok(self.store.get_newest().await?.key)
    }

    pub async fn get_verifying_key(&self, kid: &str) -> Result<VerifyingKey, store::Error> {
        let key = self.store.list(vec![kid]).await?;
        Ok(key[0].key.verifying_key().clone())
    }
}
