//! Decode bit markers evaluated by the exact linked OpenSSL headers.
//! This also works with expression macros and when cross compiling.
pub fn parse(expanded: &str) -> Result<u64, String> {
    let mut active = false;
    let mut complete = false;
    let mut seen = 0u32;
    let mut version = 0u64;
    for line in expanded.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "TARANTOOL_OPENSSL_API_BEGIN" {
            if active || complete {
                return Err("duplicate compiled OpenSSL version block".into());
            }
            active = true;
        } else if line == "TARANTOOL_OPENSSL_API_END" {
            if !active || seen != u32::MAX {
                return Err("incomplete compiled OpenSSL version block".into());
            }
            active = false;
            complete = true;
        } else if let Some(marker) = line.strip_prefix("TARANTOOL_OPENSSL_API_BIT_") {
            if !active {
                return Err("OpenSSL version bit outside its block".into());
            }
            let (bit, value) = marker
                .split_once('_')
                .ok_or_else(|| format!("invalid compiled OpenSSL version marker {line:?}"))?;
            let bit = bit
                .parse::<u32>()
                .ok()
                .filter(|bit| *bit < 32)
                .ok_or_else(|| format!("invalid compiled OpenSSL bit {bit:?}"))?;
            if seen & (1u32 << bit) != 0 || !matches!(value, "0" | "1") {
                return Err(format!(
                    "duplicate or invalid compiled OpenSSL bit {line:?}"
                ));
            }
            seen |= 1u32 << bit;
            if value == "1" {
                version |= 1u64 << bit;
            }
        } else if active {
            return Err(format!(
                "unexpected compiled OpenSSL version token {line:?}"
            ));
        }
    }
    if active || !complete {
        return Err("missing or unterminated compiled OpenSSL version block".into());
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(number: u32) -> String {
        let mut result =
            String::from("unrelated header declaration\nTARANTOOL_OPENSSL_API_BEGIN\n");
        for bit in 0..32 {
            result.push_str(&format!(
                "# 19 \"openssl/opensslv.h\" 3 4\nTARANTOOL_OPENSSL_API_BIT_{bit}_{}\n",
                (number >> bit) & 1
            ));
        }
        result.push_str("TARANTOOL_OPENSSL_API_END\nTARANTOOL_TLS_PROTOCOL_772\n");
        result
    }

    #[test]
    fn preserves_legacy_patch_status_and_modern_versions() {
        // 1.1.1w, a legacy beta, OpenSSL 3.0.13, 3.5.0 and the high-bit boundary.
        for number in [
            0x1010_117f,
            0x1010_0012,
            0x3000_00d0,
            0x3050_0000,
            0x8000_0000,
        ] {
            assert_eq!(parse(&expanded(number)).unwrap(), u64::from(number));
        }
    }

    #[test]
    fn rejects_missing_duplicate_and_malformed_markers() {
        let good = expanded(0x3000_00d0);
        for invalid in [
            String::new(),
            good.replace("TARANTOOL_OPENSSL_API_BEGIN", ""),
            good.replace("TARANTOOL_OPENSSL_API_END", ""),
            good.replace("TARANTOOL_OPENSSL_API_BIT_31_0", ""),
            good.replace(
                "TARANTOOL_OPENSSL_API_BIT_31_0",
                "TARANTOOL_OPENSSL_API_BIT_30_0",
            ),
            good.replace(
                "TARANTOOL_OPENSSL_API_BIT_31_0",
                "TARANTOOL_OPENSSL_API_BIT_32_0",
            ),
            good.replace(
                "TARANTOOL_OPENSSL_API_BIT_31_0",
                "TARANTOOL_OPENSSL_API_BIT_31_2",
            ),
            good.replace(
                "TARANTOOL_OPENSSL_API_BIT_31_0",
                "TARANTOOL_OPENSSL_API_BIT_31_1_0",
            ),
            good.replace(
                "TARANTOOL_OPENSSL_API_BIT_31_0",
                "TARANTOOL_OPENSSL_API_BIT_x_0",
            ),
            good.replace("TARANTOOL_OPENSSL_API_BIT_31_0", "unexpanded macro"),
            format!("{good}{good}"),
            format!("{good}TARANTOOL_OPENSSL_API_BIT_31_0\n"),
        ] {
            assert!(parse(&invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
