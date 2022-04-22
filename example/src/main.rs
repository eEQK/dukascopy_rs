use dukascopy_rs::DukascopyService;
use futures::TryStreamExt;
use time::macros::datetime;

#[tokio::main]
async fn main() {
    DukascopyService::default()
        .download_ticks(
            String::from("EURGBP"),
            // only full hours are supported for now
            datetime!(2020-03-12 13:00),
            datetime!(2020-03-12 15:00),
        )
        .try_for_each(|e| async move {
            let serialized = serde_json::to_string(&e).unwrap();
            println!("{}", serialized);

            println!("{}", e);

            Ok(())
        })
        .await
        .unwrap_or(());
}
