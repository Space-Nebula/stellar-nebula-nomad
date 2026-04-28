/// Gas-optimized computation utilities
/// Provides efficient algorithms and patterns for common computations
use soroban_sdk::{Env, Vec, BytesN};

/// Fast hash for small inputs (optimized for gas)
pub fn fast_hash_u64(env: &Env, input: u64) -> u64 {
    // Simple but fast hash for u64
    let mut hash = input;
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51afd7ed558ccd);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xc4ceb9fe1a85ec53);
    hash ^= hash >> 33;
    hash
}

/// Optimized sum for Vec<u32> with loop unrolling
pub fn sum_vec_u32_optimized(values: &Vec<u32>) -> u64 {
    let len = values.len();
    let mut sum: u64 = 0;
    let mut i = 0;
    
    // Process 4 elements at a time (loop unrolling)
    while i + 4 <= len {
        sum += values.get_unchecked(i) as u64;
        sum += values.get_unchecked(i + 1) as u64;
        sum += values.get_unchecked(i + 2) as u64;
        sum += values.get_unchecked(i + 3) as u64;
        i += 4;
    }
    
    // Handle remaining elements
    while i < len {
        sum += values.get_unchecked(i) as u64;
        i += 1;
    }
    
    sum
}

/// Optimized average calculation
pub fn average_u32_optimized(values: &Vec<u32>) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let sum = sum_vec_u32_optimized(values);
    (sum / values.len() as u64) as u32
}

/// Fast min/max finding with single pass
pub fn min_max_u32(values: &Vec<u32>) -> (u32, u32) {
    if values.is_empty() {
        return (0, 0);
    }
    
    let mut min = values.get_unchecked(0);
    let mut max = min;
    
    for i in 1..values.len() {
        let val = values.get_unchecked(i);
        if val < min {
            min = val;
        }
        if val > max {
            max = val;
        }
    }
    
    (min, max)
}

/// Optimized count of non-zero elements
pub fn count_nonzero_u32(values: &Vec<u32>) -> u32 {
    let len = values.len();
    let mut count = 0u32;
    let mut i = 0;
    
    // Unrolled loop
    while i + 4 <= len {
        if values.get_unchecked(i) != 0 { count += 1; }
        if values.get_unchecked(i + 1) != 0 { count += 1; }
        if values.get_unchecked(i + 2) != 0 { count += 1; }
        if values.get_unchecked(i + 3) != 0 { count += 1; }
        i += 4;
    }
    
    while i < len {
        if values.get_unchecked(i) != 0 { count += 1; }
        i += 1;
    }
    
    count
}

/// Efficient filtering without allocations
pub fn filter_nonzero_u32(env: &Env, values: &Vec<u32>) -> Vec<u32> {
    let count = count_nonzero_u32(values);
    let mut result = Vec::new(env);
    
    if count == 0 {
        return result;
    }
    
    for i in 0..values.len() {
        let val = values.get_unchecked(i);
        if val != 0 {
            result.push_back(val);
        }
    }
    
    result
}

/// Optimized binary search (assumes sorted input)
pub fn binary_search_u32(values: &Vec<u32>, target: u32) -> Option<u32> {
    let mut left = 0;
    let mut right = values.len();
    
    while left < right {
        let mid = left + (right - left) / 2;
        let mid_val = values.get_unchecked(mid);
        
        if mid_val == target {
            return Some(mid);
        } else if mid_val < target {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    
    None
}

/// Fast power of 2 check
#[inline]
pub fn is_power_of_two(n: u32) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

/// Fast log2 for power of 2 numbers
#[inline]
pub fn log2_pow2(n: u32) -> u32 {
    n.trailing_zeros()
}

/// Optimized clamp function
#[inline]
pub fn clamp_u32(value: u32, min: u32, max: u32) -> u32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Efficient percentage calculation avoiding overflow
pub fn percentage_u32(value: u32, percentage: u32) -> u32 {
    ((value as u64 * percentage as u64) / 100) as u32
}

/// Optimized weighted average
pub fn weighted_average_u32(values: &Vec<u32>, weights: &Vec<u32>) -> u32 {
    if values.is_empty() || values.len() != weights.len() {
        return 0;
    }
    
    let mut weighted_sum: u64 = 0;
    let mut weight_sum: u64 = 0;
    
    for i in 0..values.len() {
        let val = values.get_unchecked(i) as u64;
        let weight = weights.get_unchecked(i) as u64;
        weighted_sum += val * weight;
        weight_sum += weight;
    }
    
    if weight_sum == 0 {
        return 0;
    }
    
    (weighted_sum / weight_sum) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_sum_optimized() {
        let env = Env::default();
        let values = soroban_sdk::vec![&env, 1u32, 2, 3, 4, 5];
        assert_eq!(sum_vec_u32_optimized(&values), 15);
    }

    #[test]
    fn test_min_max() {
        let env = Env::default();
        let values = soroban_sdk::vec![&env, 5u32, 2, 8, 1, 9];
        assert_eq!(min_max_u32(&values), (1, 9));
    }

    #[test]
    fn test_count_nonzero() {
        let env = Env::default();
        let values = soroban_sdk::vec![&env, 1u32, 0, 3, 0, 5];
        assert_eq!(count_nonzero_u32(&values), 3);
    }

    #[test]
    fn test_is_power_of_two() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(8));
        assert!(!is_power_of_two(3));
        assert!(!is_power_of_two(6));
    }

    #[test]
    fn test_percentage() {
        assert_eq!(percentage_u32(100, 50), 50);
        assert_eq!(percentage_u32(200, 25), 50);
        assert_eq!(percentage_u32(1000, 10), 100);
    }
}
