//! The MCCS capability string a monitor answers over DDC/CI, read for the controls it
//! offers. Windows hands it over raw (`CapabilitiesRequestAndCapabilitiesReply`), where
//! ddcutil parses it itself.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::BTreeMap;

use lbm_ipc::vcp::Choice;

use super::common::choice_label;

/// The `vcp(...)` section: every code, with the values listed after it for a
/// non-continuous one (`14(05 08 0B)`), none for a continuous one. Hex pairs are read
/// with or without spaces between them, since monitors write both; what cannot be read
/// is skipped rather than guessed.
pub(super) fn capabilities(caps: &str) -> BTreeMap<u8, Vec<Choice>> {
    let mut result = BTreeMap::new();
    let lower = caps.to_ascii_lowercase();
    let Some(start) = lower.find("vcp(") else {
        return result;
    };
    let body: Vec<char> = caps[start + 4..].chars().collect();
    let mut i = 0;
    let mut code: Option<u8> = None;
    while i < body.len() {
        let c = body[i];
        if c == ')' {
            break;
        }
        if c == '(' {
            // The values of the code just read; nested groups are kept out of them.
            let mut depth = 1;
            let mut values = String::new();
            i += 1;
            while i < body.len() && depth > 0 {
                match body[i] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    c if depth == 1 => values.push(c),
                    _ => {}
                }
                i += 1;
            }
            if let Some(code) = code {
                let choices: &mut Vec<Choice> = result.entry(code).or_default();
                for value in hex_pairs(&values) {
                    choices.push(Choice {
                        value: value.into(),
                        label: choice_label(code, value.into(), ""),
                    });
                }
            }
            code = None;
            continue;
        }
        if c.is_ascii_hexdigit() && body.get(i + 1).is_some_and(char::is_ascii_hexdigit) {
            let pair: String = body[i..i + 2].iter().collect();
            if let Ok(value) = u8::from_str_radix(&pair, 16) {
                result.entry(value).or_default();
                code = Some(value);
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    result
}

fn hex_pairs(text: &str) -> Vec<u8> {
    let digits: Vec<char> = text.chars().filter(char::is_ascii_hexdigit).collect();
    let (pairs, _) = digits.as_chunks::<2>();
    pairs
        .iter()
        .filter_map(|pair| u8::from_str_radix(&pair.iter().collect::<String>(), 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vcp_section_gives_each_code_and_the_values_of_the_listed_ones() {
        let caps = "(prot(monitor)type(LCD)model(U2720Q)cmds(01 02 03 07 0C E3 F3)\
                    vcp(02 04 10 12 14(05 08 0B) 16 60(0F 11 1B) 62 D6(01 04 05) DF)\
                    mswhql(1)mccs_ver(2.1))";
        let caps = capabilities(caps);
        assert!(caps[&0x10].is_empty());
        let inputs: Vec<u16> = caps[&0x60].iter().map(|c| c.value).collect();
        assert_eq!(inputs, [0x0f, 0x11, 0x1b]);
        assert_eq!(caps[&0x60][0].label, "DisplayPort-1");
        assert_eq!(caps[&0x60][2].label, "0x1B");
        assert_eq!(caps[&0xd6][0].label, "On");
        assert!(caps.contains_key(&0xdf));
        // The commands are not controls.
        assert!(!caps.contains_key(&0xe3));
    }

    #[test]
    fn packed_pairs_read_as_spaced_ones_and_no_section_is_no_control() {
        let caps = capabilities("(vcp(1012 60(0f11)D6(0104)))");
        assert_eq!(
            caps.keys().copied().collect::<Vec<_>>(),
            [0x10, 0x12, 0x60, 0xd6]
        );
        assert_eq!(caps[&0x60].len(), 2);
        assert!(capabilities("(prot(monitor)type(LCD))").is_empty());
    }
}
