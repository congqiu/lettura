//! Background image processor.
//!
//! Processes images in entry HTML asynchronously, downloading and storing
//! them locally or in object storage.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::models::{entry, image_process_job};
use crate::storage::ImageStorage;

/// Maximum concurrent image processing jobs.
const MAX_CONCURRENT_JOBS: usize = 4;

/// Maximum retries for failed jobs.
const MAX_RETRIES: i32 = 3;

/// Interval to check for new jobs when queue is empty.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

pub struct ImageProcessor {
    pool: sqlx::PgPool,
    storage: Arc<dyn ImageStorage>,
    semaphore: Arc<Semaphore>,
    max_image_size: usize,
}

impl ImageProcessor {
    pub fn new(pool: sqlx::PgPool, storage: Arc<dyn ImageStorage>, max_image_size: usize) -> Self {
        Self {
            pool,
            storage,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_JOBS)),
            max_image_size,
        }
    }

    /// Run the processor loop.
    pub async fn run(self: Arc<Self>) {
        loop {
            match image_process_job::claim_pending(&self.pool).await {
                Ok(Some(job)) => {
                    let processor = self.clone();
                    let permit = self
                        .semaphore
                        .clone()
                        .acquire_owned()
                        .await
                        .expect("semaphore is never closed");

                    tokio::spawn(async move {
                        processor.process_job(&job).await;
                        drop(permit);
                    });
                }
                Ok(None) => {
                    // No jobs available, wait before polling again
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                Err(e) => {
                    tracing::error!("Failed to claim image process job: {e}");
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            }
        }
    }

    async fn process_job(&self, job: &image_process_job::ImageProcessJob) {
        tracing::info!(
            job_id = %job.id,
            entry_id = %job.entry_id,
            retry_count = job.retry_count,
            "processing image job"
        );

        let processed_html = crate::storage::process_images(
            &job.original_html,
            self.storage.clone(),
            self.max_image_size,
        )
        .await;

        // Update entry content with processed HTML
        match entry::update_content_only(&self.pool, job.entry_id, &processed_html).await {
            Ok(()) => {
                if let Err(e) = image_process_job::mark_completed(&self.pool, job.id).await {
                    tracing::error!("Failed to mark job completed: {e}");
                }
                tracing::info!(
                    job_id = %job.id,
                    entry_id = %job.entry_id,
                    "image processing completed"
                );
            }
            Err(e) => {
                tracing::error!("Failed to update entry content: {e}");
                if let Err(e) = image_process_job::mark_failed(
                    &self.pool,
                    job.id,
                    &format!("Failed to update entry: {e}"),
                    MAX_RETRIES,
                )
                .await
                {
                    tracing::error!("Failed to mark job failed: {e}");
                }
            }
        }
    }
}

/// Start the image processor as a background task.
pub fn start_image_processor(
    pool: sqlx::PgPool,
    storage: Arc<dyn ImageStorage>,
    max_image_size: usize,
) -> Arc<ImageProcessor> {
    let processor = Arc::new(ImageProcessor::new(pool, storage, max_image_size));
    let processor_clone = processor.clone();

    tokio::spawn(async move {
        processor_clone.run().await;
    });

    processor
}
