//! Read pinned CPython C tables preprocessed with the linked OpenSSL headers.
//! GCC can split a macro expansion across source-location directives.
#[derive(Debug)]
pub struct ErrorNames {
    pub libraries: Vec<String>,
    pub reasons: Vec<String>,
}

pub fn parse(expanded: &str) -> Result<ErrorNames, String> {
    let mut result = ErrorNames {
        libraries: Vec::new(),
        reasons: Vec::new(),
    };
    let mut section = "";
    let mut entry = String::new();
    for line in expanded.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("static struct py_ssl_library_code library_codes[]") {
            section = "library";
        } else if line.starts_with("static struct py_ssl_error_code error_codes[]") {
            section = "reason";
        } else if line == "};" {
            if !entry.is_empty() {
                return Err(format!("unterminated {section} error table entry"));
            }
            section = "";
        } else if !section.is_empty() && (!entry.is_empty() || line.starts_with("{\"")) {
            entry.push_str(line);
            entry.push(' ');
            if !line.trim_end_matches(',').ends_with('}') {
                continue;
            }
            let fields: Vec<_> = entry
                .trim()
                .trim_end_matches(',')
                .trim_start_matches('{')
                .trim_end_matches('}')
                .split(',')
                .map(str::trim)
                .collect();
            let expected = if section == "library" { 2 } else { 3 };
            if fields.len() != expected || !fields[0].starts_with('"') || !fields[0].ends_with('"')
            {
                return Err(format!("invalid {section} error table entry: {entry}"));
            }
            if section == "library" {
                result
                    .libraries
                    .push(format!("({}, {})", number(fields[1])?, fields[0]));
            } else {
                result.reasons.push(format!(
                    "({}, {}, {})",
                    number(fields[1])?,
                    number(fields[2])?,
                    fields[0]
                ));
            }
            entry.clear();
        }
    }
    if !entry.is_empty() {
        return Err(format!("unterminated {section} error table entry"));
    }
    Ok(result)
}

fn number(field: &str) -> Result<i32, String> {
    let field = field.trim_end_matches(['L', 'U', 'l', 'u']);
    let parsed = if let Some(hex) = field
        .strip_prefix("0x")
        .or_else(|| field.strip_prefix("0X"))
    {
        i32::from_str_radix(hex, 16)
    } else {
        field.parse::<i32>()
    };
    parsed.map_err(|error| format!("invalid numeric CPython error table entry {field:?}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gcc_system_header_line_markers_preserve_complete_records() {
        // Actual GCC 13 output shape on Ubuntu/OpenSSL 3.0.13. Source line
        // numbers/flags must not be confused with either numeric table field.
        let names = parse(
            r#"
static struct py_ssl_library_code library_codes[] = {
    {"ASN1",
# 4 "src/runtime/ssl_error_data/_ssl_data_300.h" 3 4
            13
# 4 "src/runtime/ssl_error_data/_ssl_data_300.h"
                        },
    {"SSL", 20},
    {NULL, 0}
};
static struct py_ssl_error_code error_codes[] = {
    {"CERTIFICATE_VERIFY_FAILED",
# 123 "src/runtime/ssl_error_data/_ssl_data_300.h" 3 4
            20
# 123 "src/runtime/ssl_error_data/_ssl_data_300.h"
            ,
# 123 "src/runtime/ssl_error_data/_ssl_data_300.h" 3 4
            134
# 123 "src/runtime/ssl_error_data/_ssl_data_300.h"
            },
    {"UNEXPECTED_EOF_WHILE_READING", 20UL, 0x126},
    {NULL, 0, 0}
};
"#,
        )
        .unwrap();
        assert_eq!(names.libraries, ["(13, \"ASN1\")", "(20, \"SSL\")"]);
        assert_eq!(
            names.reasons,
            [
                "(20, 134, \"CERTIFICATE_VERIFY_FAILED\")",
                "(20, 294, \"UNEXPECTED_EOF_WHILE_READING\")"
            ]
        );
    }
    #[test]
    fn malformed_or_truncated_expansions_fail_with_a_diagnostic() {
        for entry in [
            "{\"SSL\",",
            "{\"SSL\", still_a_macro},",
            "{\"SSL\", 20, 1},",
        ] {
            assert!(parse(&format!(
                "static struct py_ssl_library_code library_codes[] = {{\n{entry}\n}};\n"
            ))
            .is_err());
        }
    }
}
