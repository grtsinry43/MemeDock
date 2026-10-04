use memedock_domain::{error::DomainError, ordering::SortKey};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn algorithm_v1_has_fixed_string_vectors_and_binary_order() -> TestResult {
    let center = SortKey::default();
    let after = SortKey::between(Some(&center), None)?;
    let before = SortKey::between(None, Some(&center))?;
    let between = SortKey::between(Some(&center), Some(&after))?;
    assert_eq!(center.to_string(), "80");
    assert_eq!(after.to_string(), "8180");
    assert_eq!(before.to_string(), "7f80");
    assert_eq!(between.to_string(), "817f80");
    assert!(before < center && center < between && between < after);
    let keys = [before, center, between, after];
    let strings: Vec<_> = keys.iter().map(ToString::to_string).collect();
    assert!(
        strings
            .windows(2)
            .all(|w| w[0].as_bytes() < w[1].as_bytes())
    );
    for key in keys {
        assert_eq!(key.to_string().parse::<SortKey>()?, key);
    }
    Ok(())
}

#[test]
fn equal_or_reversed_bounds_fail_without_changing_order() -> TestResult {
    let center = SortKey::default();
    let after = SortKey::between(Some(&center), None)?;
    assert_eq!(
        SortKey::between(Some(&center), Some(&center)),
        Err(DomainError::InvalidBounds)
    );
    assert_eq!(
        SortKey::between(Some(&after), Some(&center)),
        Err(DomainError::InvalidBounds)
    );
    Ok(())
}

#[test]
fn key_parser_rejects_malformed_noncanonical_and_unterminated_inputs() {
    for value in [
        "", "8", "800", "80Ff80", "ff", "gg80", "0081", "猫猫", "8猫", "٨٠",
    ] {
        assert!(value.parse::<SortKey>().is_err(), "accepted {value:?}");
    }
}

#[test]
fn repeated_insertion_and_deterministic_rebalance_preserve_order() -> TestResult {
    let left = SortKey::default();
    let mut right = SortKey::between(Some(&left), None)?;
    for _ in 0..512 {
        let middle = SortKey::between(Some(&left), Some(&right))?;
        assert!(left < middle && middle < right);
        assert!(left.to_string().as_bytes() < middle.to_string().as_bytes());
        assert!(middle.to_string().as_bytes() < right.to_string().as_bytes());
        right = middle;
    }
    for count in [0, 1, 2, 3, 60, 100, 1000] {
        let keys = SortKey::rebalance(count)?;
        assert_eq!(keys.len(), count);
        assert_eq!(keys, SortKey::rebalance(count)?);
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
        assert!(
            keys.windows(2)
                .all(|w| w[0].to_string().as_bytes() < w[1].to_string().as_bytes())
        );
        if count > 0 {
            assert!(keys.iter().all(|key| key.to_string().len() <= 8));
        }
    }
    Ok(())
}

#[test]
fn zero_prefix_keys_and_reverse_bounds_never_panic() -> TestResult {
    let pairs = [
        ("000080", "000180"),
        ("0080", "0180"),
        ("00000080", "00000180"),
        ("0080", "008180"),
        ("7f0080", "7f0180"),
    ];
    for (left, right) in pairs {
        let left: SortKey = left.parse()?;
        let right: SortKey = right.parse()?;
        let middle = SortKey::between(Some(&left), Some(&right))?;
        assert!(left < middle && middle < right);
        assert_eq!(
            SortKey::between(Some(&right), Some(&left)),
            Err(DomainError::InvalidBounds)
        );
    }
    for keys in SortKey::rebalance(1000)?.windows(2) {
        let middle = SortKey::between(Some(&keys[0]), Some(&keys[1]))?;
        assert!(keys[0] < middle && middle < keys[1]);
    }
    Ok(())
}
