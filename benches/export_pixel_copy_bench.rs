use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_pixel_copy(c: &mut Criterion) {
    // Typical 1080p frame size
    let w = 1920_usize;
    let h = 1080_usize;
    let stride = w * 4; // Assuming 4 bytes per pixel (RGBA)

    // Simulate input raw_data
    let raw_data = vec![255u8; h * stride];

    let mut group = c.benchmark_group("pixel_copy_1080p");

    group.bench_function("allocate_every_frame", |b| {
        b.iter(|| {
            let mut packed_data = vec![0u8; w * h * 4];
            for y in 0..h {
                let src_start = y * stride;
                let dst_start = y * (w * 4);
                packed_data[dst_start..dst_start + (w * 4)]
                    .copy_from_slice(&raw_data[src_start..src_start + (w * 4)]);
            }
            black_box(packed_data);
        })
    });

    group.bench_function("reuse_buffer", |b| {
        let mut packed_data = vec![0u8; w * h * 4];
        b.iter(|| {
            for y in 0..h {
                let src_start = y * stride;
                let dst_start = y * (w * 4);
                packed_data[dst_start..dst_start + (w * 4)]
                    .copy_from_slice(&raw_data[src_start..src_start + (w * 4)]);
            }
            black_box(&packed_data);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_pixel_copy);
criterion_main!(benches);
