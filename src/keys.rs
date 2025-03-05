use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::{future::Future, sync::Arc};

use crate::store::{
    self,
    keys::{Key, Store as KeyStore},
};

pub trait KeyManager: Send + Sync + 'static {
    fn rotate_key(&self, clear: bool) -> impl Future<Output = Result<(), store::Error>> + Send;
    fn get_signing_key(
        &self,
    ) -> impl Future<Output = Result<(String, SigningKey), store::Error>> + Send;
    fn get_verifying_key(
        &self,
        kid: &str,
    ) -> impl Future<Output = Result<VerifyingKey, store::Error>> + Send;
}

#[derive(Clone)]
pub struct StoreKeyManager<Store: KeyStore> {
    store: Arc<Store>,
}

impl<Store: KeyStore> StoreKeyManager<Store> {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

impl<Store: KeyStore> KeyManager for StoreKeyManager<Store> {
    async fn rotate_key(&self, clear: bool) -> Result<(), store::Error> {
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
            key: SigningKey::generate(&mut OsRng),
            created_at: Utc::now(),
        };

        let _ = self.store.insert(key).await?;

        if clear {
            self.store.delete(clear_keys).await?;
        }

        Ok(())
    }

    async fn get_signing_key(&self) -> Result<(String, SigningKey), store::Error> {
        let key_data = self.store.get_newest().await?;

        Ok((key_data.id, key_data.key))
    }

    async fn get_verifying_key(&self, kid: &str) -> Result<VerifyingKey, store::Error> {
        let key = self.store.list(vec![kid]).await?;
        if key.is_empty() {
            return Err(store::Error::IdDoesNotExist(kid.to_string()));
        }

        Ok(key[0].key.verifying_key())
    }
}
