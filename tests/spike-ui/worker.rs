use super::*;
#[test]
fn newest_generation_wins_and_shutdown_does_not_lose_wakeup() {
    for _ in 0..8 {
        let worker = Worker::new(
            feishin_spike_model::records(100_000),
            egui::Context::default(),
        );
        for generation in 1..=20 {
            worker.submit(
                generation,
                Query {
                    text: generation.to_string(),
                    ..Default::default()
                },
            );
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some((generation, rows)) = worker.take()
                && generation == 20
            {
                let rows = rows.unwrap();
                assert!(rows.iter().all(|row| match row {
                    ViewRow::Track(id) => format!("Track {id:06}").contains("20"),
                    _ => false,
                }));
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "latest result was not delivered"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        drop(worker);
    }
}
