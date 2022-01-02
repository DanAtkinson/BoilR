mod origin;
mod settings;
mod steam;
mod steamgriddb;
mod sync;

#[cfg(feature = "ui")]
mod ui;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "ui")]
    {
        ui::run_ui().await
    }
    #[cfg(not(feature = "ui"))]
    {
        let settings = settings::Settings::new()?;
        sync::run_sync(&settings).await
    }
}
