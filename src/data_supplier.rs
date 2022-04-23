use async_trait::async_trait;
use bytes::Bytes;

use crate::error::Kind;

/// An interface used by [DukascopyService](crate::DukascopyService) to fetch
/// the data for further processing
#[async_trait]
pub trait DataSupplier {
    /// Fetches the data from the given URL
    async fn fetch(&self, url: &str) -> Result<Option<Bytes>, crate::error::Error>;
}

pub(crate) struct ReqwestDataSupplier {
    _priv: (),
}

impl ReqwestDataSupplier {
    pub fn new() -> Self {
        ReqwestDataSupplier { _priv: () }
    }
}

#[async_trait]
impl DataSupplier for ReqwestDataSupplier {
    async fn fetch(&self, url: &str) -> Result<Option<Bytes>, crate::error::Error> {
        let response = reqwest::get(url).await;

        match response {
            Ok(resp) => match resp.bytes().await {
                Ok(bytes) if bytes.len() == 0 => Ok(None),
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) => Err(crate::error::Error {
                    kind: Kind::Network,
                    inner: Box::new(error),
                }),
            },
            // it is a valid case for the server to return a 404 - it means there were no events
            // during the requested time interval
            Err(error) if error.status().unwrap().as_u16() == 404 => Ok(None),
            Err(error) => Err(crate::error::Error {
                kind: Kind::Network,
                inner: Box::new(error),
            }),
        }
    }
}

pub(crate) mod tests {
    use std::{fs::File, io::Read, path::Path};

    use async_trait::async_trait;
    use bytes::Bytes;

    use super::DataSupplier;

    pub struct TestResourceDataSupplier;

    #[async_trait]
    impl DataSupplier for TestResourceDataSupplier {
        async fn fetch(&self, url: &str) -> Result<Option<Bytes>, crate::error::Error> {
            let file_name = url.split('/').last().unwrap();
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("test-resources")
                .join(file_name);

            Ok(File::open(path)
                .map(|file| {
                    let bytes = file.bytes().map(|b| b.unwrap());
                    Some(Bytes::from_iter(bytes))
                })
                .unwrap_or(None))
        }
    }

    pub struct InMemoryDataSupplier {
        pub data: Option<Bytes>,
    }

    #[async_trait]
    impl DataSupplier for InMemoryDataSupplier {
        async fn fetch(&self, _url: &str) -> Result<Option<Bytes>, crate::error::Error> {
            Ok(self.data.clone())
        }
    }
}
