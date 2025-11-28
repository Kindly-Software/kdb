#![feature(test)]

extern crate kindly_engine;
extern crate test;

use kindly_engine::terrain::{TerrainGridCapsule, TerrainSnapshot};
use test::{black_box, Bencher};

// Baseline (kindly-hub Ryzen 9 6900HX, nightly, simd-los+avx2-los): simd_path ~36.5µs, heavy_grid ~108.4µs, battlefield ~190.6µs.

/// Exercise LOS sampling over a dense grid to stress the SIMD averaging path.
#[bench]
fn los_cover_simd_path(b: &mut Bencher) {
    let mut grid = TerrainGridCapsule::new(
        256,
        256,
        TerrainSnapshot {
            height_mm: 0,
            slope_q16: 0,
            cover_q16: 12_000,
            mud_q16: 2_000,
            material: 1,
        },
    );

    // Scatter higher cover tiles to force non-uniform samples.
    for i in (0..256u32).step_by(31) {
        let x = i;
        let y = (i.wrapping_mul(7).wrapping_add(13)) & 255;
        grid.set_tile(
            x,
            y,
            TerrainSnapshot {
                height_mm: 10,
                slope_q16: 500,
                cover_q16: 48_000,
                mud_q16: 22_000,
                material: 2,
            },
        );
    }

    b.iter(|| {
        let mut acc: u32 = 0;
        for offset in 0..64u32 {
            let start = (offset, (offset.wrapping_mul(5)) & 255);
            let end = (
                255 - offset,
                (offset.wrapping_mul(9).wrapping_add(31)) & 255,
            );
            let (clear, cover): (bool, u32) = grid.los_clear_gather_free(start, end);
            black_box(clear);
            acc ^= cover.wrapping_add(offset as u32);
        }
        black_box(acc);
    });
}

/// Heavier workload: larger grid and more rays to gauge throughput scaling.
#[bench]
fn los_cover_heavy_grid(b: &mut Bencher) {
    let mut grid = TerrainGridCapsule::new(
        512,
        512,
        TerrainSnapshot {
            height_mm: 0,
            slope_q16: 0,
            cover_q16: 8_000,
            mud_q16: 1_000,
            material: 1,
        },
    );

    // Plant dense obstacles across multiple diagonals.
    for i in (0..512u32).step_by(19) {
        let x = i;
        let y = (i.wrapping_mul(11).wrapping_add(23)) & 511;
        grid.set_tile(
            x,
            y,
            TerrainSnapshot {
                height_mm: 25,
                slope_q16: 1_200,
                cover_q16: 52_000,
                mud_q16: 30_000,
                material: 3,
            },
        );
    }

    b.iter(|| {
        let mut acc: u32 = 0;
        for offset in 0..128u32 {
            let start = ((offset * 3) & 511, (offset * 5 + 17) & 511);
            let end = (
                511u32.wrapping_sub(offset * 2) & 511,
                511u32.wrapping_sub(offset * 7) & 511,
            );
            let (clear, cover): (bool, u32) = grid.los_clear_gather_free(start, end);
            black_box(clear);
            acc ^= cover.wrapping_add(offset as u32);
        }
        black_box(acc);
    });
}

/// Battlefield-scale workload: long rays across wide lines to mimic artillery/line-of-battle sight checks.
#[bench]
fn los_cover_battlefield(b: &mut Bencher) {
    let mut grid = TerrainGridCapsule::new(
        1024,
        256,
        TerrainSnapshot {
            height_mm: 0,
            slope_q16: 0,
            cover_q16: 6_000,
            mud_q16: 1_500,
            material: 1,
        },
    );

    // Lay down ridge lines and forest belts every ~128m to force varied LOS occlusion.
    for x in (0..1024u32).step_by(128) {
        for y in 0..256u32 {
            let cover = if (y / 32) % 2 == 0 { 50_000 } else { 18_000 };
            grid.set_tile(
                x,
                y,
                TerrainSnapshot {
                    height_mm: 1200,
                    slope_q16: 2_400,
                    cover_q16: cover,
                    mud_q16: 28_000,
                    material: 4,
                },
            );
        }
    }

    b.iter(|| {
        let mut acc: u32 = 0;
        // Fire 192 long-range rays (~800-1000m) across the width.
        for idx in 0..192u32 {
            let start = (idx * 3 % 1024, (idx * 5 + 17) % 256);
            let end = ((start.0 + 800) % 1024, (start.1 + 80 + idx * 2) % 256);
            let (clear, cover): (bool, u32) = grid.los_clear_gather_free(start, end);
            black_box(clear);
            acc ^= cover.wrapping_add(idx as u32);
        }
        black_box(acc);
    });
}
