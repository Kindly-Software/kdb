// Simpler DCT with post-scaling
#![feature(portable_simd)]

fn dct_1d_8point(input: &[i16; 8]) -> [i16; 8] {
    const C1: i32 = 16069;
    const C2: i32 = 15137;
    const C3: i32 = 13623;
    const C4: i32 = 11585;
    const C5: i32 = 9102;
    const C6: i32 = 6270;
    const C7: i32 = 3196;

    let x = input.map(|v| v as i32);

    let s0 = x[0] + x[7];
    let s1 = x[1] + x[6];
    let s2 = x[2] + x[5];
    let s3 = x[3] + x[4];
    let d0 = x[0] - x[7];
    let d1 = x[1] - x[6];
    let d2 = x[2] - x[5];
    let d3 = x[3] - x[4];

    let e0 = s0 + s3;
    let e1 = s1 + s2;
    let e2 = s0 - s3;
    let e3 = s1 - s2;

    // Chen-Wang butterfly (no additional scaling)
    let mut y0 = ((e0 + e1) * C4) >> 14;
    let mut y4 = ((e0 - e1) * C4) >> 14;
    let mut y2 = (e2 * C2 + e3 * C6) >> 14;
    let mut y6 = (e2 * C6 - e3 * C2) >> 14;

    let mut y1 = (d0 * C1 + d1 * C3 + d2 * C5 + d3 * C7) >> 14;
    let mut y3 = (d0 * C3 - d1 * C7 - d2 * C1 - d3 * C5) >> 14;
    let mut y5 = (d0 * C5 - d1 * C1 + d2 * C7 + d3 * C3) >> 14;
    let mut y7 = (d0 * C7 - d1 * C5 + d2 * C3 - d3 * C1) >> 14;

    // Apply orthonormal scaling: DC *= 1/sqrt(2), AC *= 1
    // In integer: DC *= 11585/16384 (same as C4)
    y0 = (y0 * C4) >> 14;

    [y0 as i16, y1 as i16, y2 as i16, y3 as i16,
     y4 as i16, y5 as i16, y6 as i16, y7 as i16]
}

fn idct_1d_8point(input: &[i16; 8]) -> [i16; 8] {
    const C1: i32 = 16069;
    const C2: i32 = 15137;
    const C3: i32 = 13623;
    const C4: i32 = 11585;
    const C5: i32 = 9102;
    const C6: i32 = 6270;
    const C7: i32 = 3196;

    let mut x = input.map(|v| v as i32);

    // Inverse orthonormal scaling: DC *= sqrt(2)
    x[0] = (x[0] * 16384) / C4;

    let x0 = x[0];
    let x1 = x[1];
    let x2 = x[2];
    let x3 = x[3];
    let x4 = x[4];
    let x5 = x[5];
    let x6 = x[6];
    let x7 = x[7];

    // Inverse butterfly
    let t1 = (x1 * C1 + x3 * C3 + x5 * C5 + x7 * C7) >> 14;
    let t3 = (x1 * C3 - x3 * C7 - x5 * C1 - x7 * C5) >> 14;
    let t5 = (x1 * C5 - x3 * C1 + x5 * C7 + x7 * C3) >> 14;
    let t7 = (x1 * C7 - x3 * C5 + x5 * C3 - x7 * C1) >> 14;

    let t0 = (x0 * C4) >> 14;
    let t4 = (x4 * C4) >> 14;
    let t2 = (x2 * C2 + x6 * C6) >> 14;
    let t6 = (x6 * C2 - x2 * C6) >> 14;

    let e0 = t0 + t4;
    let e1 = t0 - t4;
    let e2 = t2 + t6;
    let e3 = t2 - t6;

    let s0 = e0 + e2;
    let s1 = e1 + e3;
    let s2 = e1 - e3;
    let s3 = e0 - e2;

    let y0 = s0 + t1;
    let y1 = s1 + t3;
    let y2 = s2 + t5;
    let y3 = s3 + t7;
    let y4 = s3 - t7;
    let y5 = s2 - t5;
    let y6 = s1 - t3;
    let y7 = s0 - t1;

    [y0 as i16, y1 as i16, y2 as i16, y3 as i16,
     y4 as i16, y5 as i16, y6 as i16, y7 as i16]
}

fn main() {
    println!("Test 1: DC Coefficient");
    let input = [1i16; 8];
    let output = dct_1d_8point(&input);
    println!("Input: all 1s");
    println!("Output: {:?}", output);
    println!("DC: {} (expected ~6-7)", output[0]);

    println!("\nTest 2: Energy Conservation");
    let input2: [i16; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
    let output2 = dct_1d_8point(&input2);
    let energy_in: i32 = input2.iter().map(|&x| (x as i32) * (x as i32)).sum();
    let energy_out: i32 = output2.iter().map(|&x| (x as i32) * (x as i32)).sum();
    let ratio = energy_out as f64 / energy_in as f64;
    println!("Energy in: {}", energy_in);
    println!("Energy out: {}", energy_out);
    println!("Ratio: {} (expected ~1.0)", ratio);

    println!("\nTest 3: Invertibility");
    let input3: [i16; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
    let forward = dct_1d_8point(&input3);
    let inverse = idct_1d_8point(&forward);
    println!("Input:   {:?}", input3);
    println!("Inverse: {:?}", inverse);
    let max_error = input3.iter().zip(inverse.iter())
        .map(|(a, b)| (b - a).abs()).max().unwrap();
    println!("Max error: {}", max_error);
}
