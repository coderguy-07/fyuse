//! GGUF-compatible quantize/dequantize math for Q4_0 and Q8_0 formats.
//!
//! Block sizes follow the llama.cpp convention:
//! - Q4_0: 32 floats -> 18 bytes (2-byte f16 scale + 16 bytes of 4-bit values)
//! - Q8_0: 32 floats -> 34 bytes (2-byte f16 scale + 32 bytes of 8-bit values)

use tracing::info;

/// Q4_0 block size: 32 weights per block.
const Q4_0_BLOCK_SIZE: usize = 32;
/// Q4_0 encoded block: 2 bytes scale + 16 bytes data = 18 bytes.
const Q4_0_ENCODED_BLOCK: usize = 18;

/// Q8_0 block size: 32 weights per block.
const Q8_0_BLOCK_SIZE: usize = 32;
/// Q8_0 encoded block: 2 bytes scale + 32 bytes data = 34 bytes.
const Q8_0_ENCODED_BLOCK: usize = 34;

/// Quantize f32 weights to Q4_0 format.
///
/// Each block of 32 floats is encoded as:
/// - 2 bytes: f16 scale (max absolute value / 7.0)
/// - 16 bytes: 32 x 4-bit signed values packed into pairs
///
/// Input length must be a multiple of 32. If not, trailing values are zero-padded.
pub fn quantize_q4_0(weights: &[f32]) -> Vec<u8> {
    info!(
        num_weights = weights.len(),
        "Quantizing weights to Q4_0 format"
    );

    let num_blocks = weights.len().div_ceil(Q4_0_BLOCK_SIZE);
    let mut output = Vec::with_capacity(num_blocks * Q4_0_ENCODED_BLOCK);

    for block_idx in 0..num_blocks {
        let start = block_idx * Q4_0_BLOCK_SIZE;
        let end = (start + Q4_0_BLOCK_SIZE).min(weights.len());

        // Find max absolute value in block
        let mut amax: f32 = 0.0;
        for &w in &weights[start..end] {
            let abs = w.abs();
            if abs > amax {
                amax = abs;
            }
        }

        // Scale: maps [-amax, amax] to [-8, 7] (4-bit signed range)
        let scale = if amax > 0.0 { amax / 7.0 } else { 0.0 };
        let inv_scale = if scale > 0.0 { 1.0 / scale } else { 0.0 };

        // Write scale as f16 (stored as 2 bytes via half-precision)
        let scale_f16 = f16_from_f32(scale);
        output.extend_from_slice(&scale_f16.to_le_bytes());

        // Quantize and pack pairs of 4-bit values
        for pair_idx in 0..(Q4_0_BLOCK_SIZE / 2) {
            let idx0 = start + pair_idx * 2;
            let idx1 = start + pair_idx * 2 + 1;

            let w0 = if idx0 < weights.len() {
                weights[idx0]
            } else {
                0.0
            };
            let w1 = if idx1 < weights.len() {
                weights[idx1]
            } else {
                0.0
            };

            let q0 = quantize_nibble(w0 * inv_scale);
            let q1 = quantize_nibble(w1 * inv_scale);

            // Pack: low nibble = q0 + 8, high nibble = q1 + 8
            // Offset by 8 to store in unsigned 4-bit range [0, 15]
            let byte = ((q0 + 8) as u8) | (((q1 + 8) as u8) << 4);
            output.push(byte);
        }
    }

    output
}

/// Dequantize Q4_0 data back to f32 weights.
pub fn dequantize_q4_0(data: &[u8]) -> Vec<f32> {
    if data.len() < Q4_0_ENCODED_BLOCK {
        return Vec::new();
    }

    let num_blocks = data.len() / Q4_0_ENCODED_BLOCK;
    let mut output = Vec::with_capacity(num_blocks * Q4_0_BLOCK_SIZE);

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q4_0_ENCODED_BLOCK;

        // Read scale
        let scale_bytes = [data[block_start], data[block_start + 1]];
        let scale = f32_from_f16(u16::from_le_bytes(scale_bytes));

        // Unpack pairs
        for pair_idx in 0..(Q4_0_BLOCK_SIZE / 2) {
            let byte = data[block_start + 2 + pair_idx];

            let q0 = (byte & 0x0F) as i8 - 8;
            let q1 = ((byte >> 4) & 0x0F) as i8 - 8;

            output.push(q0 as f32 * scale);
            output.push(q1 as f32 * scale);
        }
    }

    output
}

/// Quantize f32 weights to Q8_0 format.
///
/// Each block of 32 floats is encoded as:
/// - 2 bytes: f16 scale (max absolute value / 127.0)
/// - 32 bytes: 32 x 8-bit signed values
pub fn quantize_q8_0(weights: &[f32]) -> Vec<u8> {
    info!(
        num_weights = weights.len(),
        "Quantizing weights to Q8_0 format"
    );

    let num_blocks = weights.len().div_ceil(Q8_0_BLOCK_SIZE);
    let mut output = Vec::with_capacity(num_blocks * Q8_0_ENCODED_BLOCK);

    for block_idx in 0..num_blocks {
        let start = block_idx * Q8_0_BLOCK_SIZE;
        let end = (start + Q8_0_BLOCK_SIZE).min(weights.len());

        // Find max absolute value
        let mut amax: f32 = 0.0;
        for &w in &weights[start..end] {
            let abs = w.abs();
            if abs > amax {
                amax = abs;
            }
        }

        let scale = if amax > 0.0 { amax / 127.0 } else { 0.0 };
        let inv_scale = if scale > 0.0 { 1.0 / scale } else { 0.0 };

        // Write scale as f16
        let scale_f16 = f16_from_f32(scale);
        output.extend_from_slice(&scale_f16.to_le_bytes());

        // Quantize each value to i8
        for i in 0..Q8_0_BLOCK_SIZE {
            let idx = start + i;
            let w = if idx < weights.len() {
                weights[idx]
            } else {
                0.0
            };
            let q = (w * inv_scale).round().clamp(-128.0, 127.0) as i8;
            output.push(q as u8);
        }
    }

    output
}

/// Dequantize Q8_0 data back to f32 weights.
pub fn dequantize_q8_0(data: &[u8]) -> Vec<f32> {
    if data.len() < Q8_0_ENCODED_BLOCK {
        return Vec::new();
    }

    let num_blocks = data.len() / Q8_0_ENCODED_BLOCK;
    let mut output = Vec::with_capacity(num_blocks * Q8_0_BLOCK_SIZE);

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q8_0_ENCODED_BLOCK;

        let scale_bytes = [data[block_start], data[block_start + 1]];
        let scale = f32_from_f16(u16::from_le_bytes(scale_bytes));

        for i in 0..Q8_0_BLOCK_SIZE {
            let q = data[block_start + 2 + i] as i8;
            output.push(q as f32 * scale);
        }
    }

    output
}

/// Clamp and round a float to a 4-bit signed integer [-8, 7].
fn quantize_nibble(v: f32) -> i8 {
    v.round().clamp(-8.0, 7.0) as i8
}

/// Convert f32 to f16 (stored as u16). Simplified conversion.
fn f16_from_f32(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = (bits >> 31) & 1;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x7FFFFF;

    if exp == 0 {
        // Zero or denormalized -> f16 zero
        return (sign << 15) as u16;
    }

    if exp == 0xFF {
        // Inf or NaN
        if mantissa == 0 {
            return ((sign << 15) | 0x7C00) as u16;
        } else {
            return ((sign << 15) | 0x7C00 | (mantissa >> 13)) as u16;
        }
    }

    let new_exp = exp - 127 + 15;
    if new_exp >= 31 {
        // Overflow -> Inf
        return ((sign << 15) | 0x7C00) as u16;
    }
    if new_exp <= 0 {
        // Underflow -> zero
        return (sign << 15) as u16;
    }

    ((sign << 15) | ((new_exp as u32) << 10) | (mantissa >> 13)) as u16
}

/// Convert f16 (stored as u16) to f32.
fn f32_from_f16(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mantissa = (h & 0x3FF) as u32;

    if exp == 0 {
        if mantissa == 0 {
            return f32::from_bits(sign << 31);
        }
        // Denormalized f16 -> normalized f32
        let mut m = mantissa;
        let mut e: i32 = -14 + 127;
        while (m & 0x400) == 0 {
            m <<= 1;
            e -= 1;
        }
        m &= 0x3FF;
        return f32::from_bits((sign << 31) | ((e as u32) << 23) | (m << 13));
    }

    if exp == 31 {
        if mantissa == 0 {
            return f32::from_bits((sign << 31) | 0x7F800000);
        }
        return f32::from_bits((sign << 31) | 0x7F800000 | (mantissa << 13));
    }

    let new_exp = exp as i32 - 15 + 127;
    f32::from_bits((sign << 31) | ((new_exp as u32) << 23) | (mantissa << 13))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q4_0_roundtrip_basic() {
        let weights: Vec<f32> = (0..32).map(|i| (i as f32 - 16.0) / 16.0).collect();
        let quantized = quantize_q4_0(&weights);
        assert_eq!(quantized.len(), Q4_0_ENCODED_BLOCK);

        let dequantized = dequantize_q4_0(&quantized);
        assert_eq!(dequantized.len(), 32);

        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!(
                (orig - deq).abs() < 0.5,
                "Q4_0 roundtrip error too large: orig={}, deq={}, diff={}",
                orig,
                deq,
                (orig - deq).abs()
            );
        }
    }

    #[test]
    fn test_q8_0_roundtrip_basic() {
        let weights: Vec<f32> = (0..32).map(|i| (i as f32 - 16.0) / 16.0).collect();
        let quantized = quantize_q8_0(&weights);
        assert_eq!(quantized.len(), Q8_0_ENCODED_BLOCK);

        let dequantized = dequantize_q8_0(&quantized);
        assert_eq!(dequantized.len(), 32);

        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!(
                (orig - deq).abs() < 0.1,
                "Q8_0 roundtrip error too large: orig={}, deq={}, diff={}",
                orig,
                deq,
                (orig - deq).abs()
            );
        }
    }

    #[test]
    fn test_q4_0_zeros() {
        let weights = vec![0.0_f32; 32];
        let quantized = quantize_q4_0(&weights);
        let dequantized = dequantize_q4_0(&quantized);
        for v in &dequantized {
            assert!(
                v.abs() < f32::EPSILON,
                "Zero input should produce zero output"
            );
        }
    }

    #[test]
    fn test_q8_0_zeros() {
        let weights = vec![0.0_f32; 32];
        let quantized = quantize_q8_0(&weights);
        let dequantized = dequantize_q8_0(&quantized);
        for v in &dequantized {
            assert!(
                v.abs() < f32::EPSILON,
                "Zero input should produce zero output"
            );
        }
    }

    #[test]
    fn test_q4_0_multiple_blocks() {
        let weights: Vec<f32> = (0..96).map(|i| (i as f32 - 48.0) / 48.0).collect();
        let quantized = quantize_q4_0(&weights);
        assert_eq!(quantized.len(), 3 * Q4_0_ENCODED_BLOCK);

        let dequantized = dequantize_q4_0(&quantized);
        assert_eq!(dequantized.len(), 96);

        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!(
                (orig - deq).abs() < 0.5,
                "Q4_0 multi-block roundtrip error: orig={}, deq={}",
                orig,
                deq
            );
        }
    }

    #[test]
    fn test_q8_0_multiple_blocks() {
        let weights: Vec<f32> = (0..96).map(|i| (i as f32 - 48.0) / 48.0).collect();
        let quantized = quantize_q8_0(&weights);
        assert_eq!(quantized.len(), 3 * Q8_0_ENCODED_BLOCK);

        let dequantized = dequantize_q8_0(&quantized);
        assert_eq!(dequantized.len(), 96);

        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!(
                (orig - deq).abs() < 0.1,
                "Q8_0 multi-block roundtrip error: orig={}, deq={}",
                orig,
                deq
            );
        }
    }

    #[test]
    fn test_dequantize_empty_input() {
        assert!(dequantize_q4_0(&[]).is_empty());
        assert!(dequantize_q8_0(&[]).is_empty());
    }

    #[test]
    fn test_f16_roundtrip() {
        let values = [0.0_f32, 1.0, -1.0, 0.5, 100.0, -0.001];
        for &v in &values {
            let h = f16_from_f32(v);
            let back = f32_from_f16(h);
            let tolerance = v.abs() * 0.01 + 0.001; // ~1% relative + small absolute
            assert!(
                (v - back).abs() < tolerance,
                "f16 roundtrip failed for {}: got {}",
                v,
                back
            );
        }
    }

    #[test]
    fn test_q4_0_property_style_roundtrip() {
        // Simulate property test with diverse inputs
        for seed in 0..20 {
            let weights: Vec<f32> = (0..64)
                .map(|i| {
                    let x = (i as f32 + seed as f32 * 7.0) * 0.1;
                    (x * 1.7).sin()
                })
                .collect();

            let quantized = quantize_q4_0(&weights);
            let dequantized = dequantize_q4_0(&quantized);

            for (orig, deq) in weights.iter().zip(dequantized.iter()) {
                assert!(
                    (orig - deq).abs() < 0.5,
                    "Q4_0 property roundtrip failed: orig={}, deq={}",
                    orig,
                    deq
                );
            }
        }
    }

    #[test]
    fn test_q8_0_property_style_roundtrip() {
        for seed in 0..20 {
            let weights: Vec<f32> = (0..64)
                .map(|i| {
                    let x = (i as f32 + seed as f32 * 7.0) * 0.1;
                    (x * 1.7).sin()
                })
                .collect();

            let quantized = quantize_q8_0(&weights);
            let dequantized = dequantize_q8_0(&quantized);

            for (orig, deq) in weights.iter().zip(dequantized.iter()) {
                assert!(
                    (orig - deq).abs() < 0.1,
                    "Q8_0 property roundtrip failed: orig={}, deq={}",
                    orig,
                    deq
                );
            }
        }
    }
}
