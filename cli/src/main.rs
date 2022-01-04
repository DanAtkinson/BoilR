use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = main::settings::Settings::new()?;
    main::sync::run_sync(&settings).await
}
