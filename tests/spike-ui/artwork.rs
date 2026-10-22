use super::*;
#[test]
fn decode_batches_and_cpu_gpu_cache_stay_bounded() {
    let ctx = egui::Context::default();
    let mut artwork = Artwork::new(ctx.clone());
    for batch in 0..4 {
        let ids: Vec<_> = (batch * BATCH..batch * BATCH + 1000).collect();
        artwork.request(&ids);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let count = artwork.result.lock().unwrap().len();
            assert!(count <= BATCH);
            if count == BATCH {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        artwork.receive(&ctx);
        assert!(artwork.cache.len() <= CAPACITY);
        assert!(artwork.texture(batch * BATCH).is_some());
    }
    assert!(artwork.texture(0).is_none());
}
