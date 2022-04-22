use byteorder::{BigEndian, ByteOrder};
use bytes::Bytes;
use crate::Tick;
use crate::{data_supplier::ReqwestDataSupplier, DataSupplier};
use crate::error::Kind;
use futures::{stream, Stream, StreamExt};
use lzma_rs::lzma_decompress;
use time::{macros::offset, Duration, PrimitiveDateTime};

/// Processes the data from a given [DataSupplier](DataSupplier)
pub struct DukascopyService {
    pub base_url: String,
    pub data_supplier: Box<dyn DataSupplier>,
}

impl Default for DukascopyService {
    fn default() -> Self {
        DukascopyService {
            base_url: "https://datafeed.dukascopy.com/datafeed".to_string(),
            data_supplier: Box::new(ReqwestDataSupplier::new()),
        }
    }
}

impl DukascopyService {
    pub fn new(base_url: String, data_supplier: Box<dyn DataSupplier>) -> DukascopyService {
        DukascopyService {
            base_url,
            data_supplier,
        }
    }

    /// Returns a stream of ticks for a given instrument and time interval.
    ///
    /// # Arguments
    ///
    /// * `instrument` - instrument for which data is fetched
    ///
    /// In order to get a ticker for a desired instrument, go to
    /// https://www.dukascopy.com/swiss/english/marketwatch/historical/
    /// and open the network tab in browser's console. For any instrument you select
    /// there will be a request made with the URL along the lines of
    /// `https://datafeed.dukascopy.com/datafeed/EURJPY/metadata/HistoryStart.bi5`.
    /// What you're looking for is the next segment after `datafeed` part.
    ///
    /// * `start` and `end` - UTC time intervals between which the data is fetched,
    /// **for now they have to be rounded to the nearest hour.**
    ///
    /// # Returned items
    ///
    /// * Ok - when data is successfully fetched and parsed
    /// * Err - when some kind of error has occurred (check the error's kind and/or inner error to see which one is it)
    pub fn download_ticks(
        &'_ self,
        instrument: String,
        start: PrimitiveDateTime,
        end: PrimitiveDateTime,
    ) -> impl Stream<Item = Result<Tick, crate::error::Error>> + '_ {
        assert_eq!(start.replace_hour(0).unwrap().as_hms_nano(), (0, 0, 0, 0));
        assert_eq!(end.replace_hour(0).unwrap().as_hms_nano(), (0, 0, 0, 0));

        stream::iter(self.compute_tick_download_times(start, end))
            .map(move |date| (date, self.generate_tick_download_url(date, &instrument)))
            .then(move |(date, url)| async move {
                self.data_supplier.fetch(&url).await.map(|b| (date, b))
            })
            .map(
                |r: Result<(PrimitiveDateTime, Option<Bytes>), crate::error::Error>| {
                    r.and_then(|(date, bytes)| {
                        self.decompress_data(bytes)
                            .map(|decompressed| (date, decompressed))
                    })
                },
            )
            .flat_map(
                |r: Result<(PrimitiveDateTime, Vec<u8>), crate::error::Error>| {
                    let items = match r {
                        Ok((date, buf)) => self
                            .buffer_to_ticks(date, buf)
                            .into_iter()
                            .map(Ok)
                            .collect(),
                        Err(e) => vec![Err(e)],
                    };

                    stream::iter(items)
                },
            )
    }

    fn generate_tick_download_url(&self, time: PrimitiveDateTime, instrument: &str) -> String {
        let (year, month, day, hour) =
            (time.year(), time.month() as u8 - 1, time.day(), time.hour());
        let base_url = &self.base_url;

        format!("{base_url}/{instrument}/{year}/{month:02}/{day:02}/{hour:02}h_ticks.bi5")
    }

    fn compute_tick_download_times(
        &self,
        start: PrimitiveDateTime,
        end: PrimitiveDateTime,
    ) -> Vec<PrimitiveDateTime> {
        let time_span = end - start;
        (0..time_span.whole_hours())
            .map(|e| start + Duration::hours(e))
            .collect()
    }

    fn decompress_data(&self, bytes: Option<Bytes>) -> Result<Vec<u8>, crate::error::Error> {
        let mut buf = Vec::<u8>::new();
        match bytes {
            Some(bytes) => match lzma_decompress(&mut bytes.as_ref(), &mut buf) {
                Ok(_) => Ok(buf),
                Err(e) => Err(crate::error::Error {
                    kind: Kind::Decode,
                    inner: Box::new(e),
                }),
            },
            None => Ok(Vec::new()),
        }
    }

    fn buffer_to_ticks(&self, date: PrimitiveDateTime, bytes: Vec<u8>) -> Vec<Tick> {
        let offset_date = date.assume_offset(offset!(UTC));
        let millis_since_epoch = offset_date.unix_timestamp();

        bytes
            .chunks(20)
            .map(|e| self.create_tick(millis_since_epoch, e))
            .collect()
    }

    fn create_tick(&self, millis_since_epoch: i64, bytes: &[u8]) -> Tick {
        debug_assert!(bytes.len() == 20);

        Tick {
            time: millis_since_epoch + BigEndian::read_u32(&bytes[0..4]) as i64,
            ask: BigEndian::read_u32(&bytes[4..8]) as f64 / 100_000f64,
            bid: BigEndian::read_u32(&bytes[8..12]) as f64 / 100_000f64,
            ask_volume: BigEndian::read_f32(&bytes[12..16]) as f64,
            bid_volume: BigEndian::read_f32(&bytes[16..20]) as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use byteorder::{BigEndian, ByteOrder};
    use bytes::Bytes;
    use futures::StreamExt;
    use lzma_rs::lzma_compress;
    use time::macros::datetime;

    use crate::{
        data_supplier::tests::{InMemoryDataSupplier, TestResourceDataSupplier},
        tick::Tick,
        DukascopyService,
    };

    #[tokio::test]
    async fn parses_bi5_file_to_ticks() {
        let mut bytes = vec![0u8; 20];

        BigEndian::write_i32(&mut bytes[0..4], 0x000000DA);
        BigEndian::write_i32(&mut bytes[4..8], 0x0001B4C7);
        BigEndian::write_i32(&mut bytes[8..12], 0x0001B4C4);
        BigEndian::write_i32(&mut bytes[12..16], 0x3F8F5C29);
        BigEndian::write_i32(&mut bytes[16..20], 0x3F400000);

        let mut compressed_bytes = Vec::<u8>::new();
        lzma_compress(&mut &bytes[..], &mut compressed_bytes).unwrap();

        let service = DukascopyService::new(
            String::from(""),
            Box::new(InMemoryDataSupplier {
                data: Some(Bytes::from_iter(compressed_bytes)),
            }),
        );

        let ticks = service
            .download_ticks(
                String::from("EURGBP"),
                datetime!(2020-03-12 01:00),
                datetime!(2020-03-12 02:00),
            )
            .collect::<Vec<Result<Tick, crate::error::Error>>>()
            .await;

        assert_eq!(ticks.len(), 1);

        let tick = ticks[0].as_ref().unwrap();
        assert_eq!(
            tick.time,
            datetime!(2020-03-12 01:00 UTC).unix_timestamp() + 218
        );
        assert_eq!(tick.ask, 1.11815);
        assert_eq!(tick.bid, 1.11812);
        assert_abs_diff_eq!(tick.ask_volume, 1.12, epsilon = 0.000_001);
        assert_eq!(tick.bid_volume, 0.75);
    }

    #[tokio::test]
    async fn fetches_and_parses_multiple_bi5_files() {
        let service =
            DukascopyService::new(String::from(""), Box::new(TestResourceDataSupplier {}));

        let ticks = service.download_ticks(
            String::from("EURGBP"),
            datetime!(2020-03-12 00:00),
            datetime!(2020-03-13 00:00),
        );

        assert_eq!(ticks.count().await, 12464)
    }
}
