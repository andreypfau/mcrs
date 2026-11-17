use std::ops::Range;

fn shebang_end(bytes: &[u8]) -> usize {
    if bytes.starts_with(b"#!") {
        bytes
            .iter()
            .position(|&b| b == b'\n')
            .unwrap_or(bytes.len())
    } else {
        0
    }
}

fn line_end(bytes: &[u8], from: usize) -> usize {
    let end = bytes
        .get(from..)
        .and_then(|rest| rest.iter().position(|&b| b == b'\n'))
        .map_or(bytes.len(), |offset| from + offset);
    if end > from && bytes.get(end - 1) == Some(&b'\r') {
        end - 1
    } else {
        end
    }
}

pub fn toml(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = shebang_end(bytes);
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'#' => {
                let end = line_end(bytes, at);
                found.push(at..end);
                at = end;
            }
            b'"' | b'\'' => at = quoted(bytes, at),
            _ => at += 1,
        }
    }
    found
}

fn quoted(bytes: &[u8], start: usize) -> usize {
    let quote = bytes.get(start).copied().unwrap_or(b'"');
    let triple = [quote, quote, quote];
    let escapes = quote == b'"';
    if bytes.get(start..start + 3) == Some(&triple) {
        let mut at = start + 3;
        while at < bytes.len() {
            if escapes && bytes.get(at) == Some(&b'\\') {
                at += 2;
            } else if bytes.get(at..at + 3) == Some(&triple) {
                return at + 3;
            } else {
                at += 1;
            }
        }
        return at;
    }
    let mut at = start + 1;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' if escapes => at += 2,
            b'\n' => return at,
            _ if byte == quote => return at + 1,
            _ => at += 1,
        }
    }
    at
}

pub fn python(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = shebang_end(bytes);
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'#' => {
                let end = line_end(bytes, at);
                found.push(at..end);
                at = end;
            }
            b'\\' => at += 2,
            b'"' | b'\'' => {
                let quote = byte;
                let triple = [quote, quote, quote];
                if bytes.get(at..at + 3) == Some(&triple) {
                    at += 3;
                    while at < bytes.len() && bytes.get(at..at + 3) != Some(&triple) {
                        at += if bytes.get(at) == Some(&b'\\') { 2 } else { 1 };
                    }
                    at += 3;
                } else {
                    at += 1;
                    while let Some(&inner) = bytes.get(at) {
                        at += 1;
                        match inner {
                            b'\\' => at += 1,
                            b'\n' => break,
                            _ if inner == quote => break,
                            _ => {}
                        }
                    }
                }
            }
            _ => at += 1,
        }
    }
    found
}

pub fn properties(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let end = line_end(bytes, start);
        let first = (start..end).find(|&i| !matches!(bytes.get(i), Some(b' ' | b'\t')));
        if let Some(first) = first.filter(|&i| matches!(bytes.get(i), Some(b'#' | b'!'))) {
            found.push(first..end);
        }
        start = next_line(bytes, end);
    }
    found
}

fn next_line(bytes: &[u8], end: usize) -> usize {
    match bytes.get(end) {
        Some(b'\r') => end + 2,
        _ => end + 1,
    }
}

pub fn yaml(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut block: Option<usize> = None;
    let mut start = 0;
    while start < bytes.len() {
        let end = line_end(bytes, start);
        let line = bytes.get(start..end).unwrap_or_default();
        let indent = line.iter().take_while(|&&b| b == b' ').count();
        let blank = line.iter().all(u8::is_ascii_whitespace);
        if let Some(parent) = block {
            if blank || indent > parent {
                start = next_line(bytes, end);
                continue;
            }
            block = None;
        }
        let mut code_end = line.len();
        let mut i = 0;
        while let Some(&byte) = line.get(i) {
            match byte {
                b'\'' => i = yaml_single_quoted(line, i + 1),
                b'"' => i = yaml_double_quoted(line, i + 1),
                b'#' if i == 0 || matches!(line.get(i - 1), Some(b' ' | b'\t')) => {
                    found.push(start + i..end);
                    code_end = i;
                    break;
                }
                _ => i += 1,
            }
        }
        let code = line.get(..code_end).unwrap_or_default().trim_ascii_end();
        if opens_block_scalar(code) {
            block = Some(indent);
        }
        start = next_line(bytes, end);
    }
    found
}

fn yaml_single_quoted(line: &[u8], mut i: usize) -> usize {
    while let Some(&byte) = line.get(i) {
        i += 1;
        if byte == b'\'' && line.get(i) != Some(&b'\'') {
            return i;
        }
        if byte == b'\'' {
            i += 1;
        }
    }
    i
}

fn yaml_double_quoted(line: &[u8], mut i: usize) -> usize {
    while let Some(&byte) = line.get(i) {
        i += 1;
        match byte {
            b'\\' => i += 1,
            b'"' => return i,
            _ => {}
        }
    }
    i
}

fn opens_block_scalar(code: &[u8]) -> bool {
    let header_start = code
        .iter()
        .rposition(|&b| b == b'|' || b == b'>')
        .filter(|&i| {
            code.get(i + 1..).is_some_and(|rest| {
                rest.iter()
                    .all(|&b| b == b'+' || b == b'-' || b.is_ascii_digit())
            })
        });
    header_start.is_some_and(|i| i == 0 || matches!(code.get(i - 1), Some(b' ' | b':' | b'-')))
}

pub fn shell(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut heredocs: Vec<(Vec<u8>, bool)> = Vec::new();
    let mut at = shebang_end(bytes);
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' => at += 2,
            b'\'' => {
                at += 1;
                while bytes.get(at).is_some_and(|&b| b != b'\'') {
                    at += 1;
                }
                at += 1;
            }
            b'$' if bytes.get(at + 1) == Some(&b'\'') => {
                at += 2;
                while let Some(&inner) = bytes.get(at) {
                    at += 1;
                    match inner {
                        b'\\' => at += 1,
                        b'\'' => break,
                        _ => {}
                    }
                }
            }
            b'"' => {
                at += 1;
                while let Some(&inner) = bytes.get(at) {
                    at += 1;
                    match inner {
                        b'\\' => at += 1,
                        b'"' => break,
                        _ => {}
                    }
                }
            }
            b'#' if at == 0
                || matches!(
                    bytes.get(at - 1),
                    Some(b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')')
                ) =>
            {
                let end = line_end(bytes, at);
                found.push(at..end);
                at = end;
            }
            b'<' if bytes.get(at + 1) == Some(&b'<') && bytes.get(at + 2) != Some(&b'<') => {
                at += 2;
                let strip_tabs = bytes.get(at) == Some(&b'-');
                if strip_tabs {
                    at += 1;
                }
                while matches!(bytes.get(at), Some(b' ' | b'\t')) {
                    at += 1;
                }
                let mut delimiter = Vec::new();
                while let Some(&b) = bytes.get(at) {
                    if b.is_ascii_whitespace() || b";&|<>()".contains(&b) {
                        break;
                    }
                    if !b"'\"\\".contains(&b) {
                        delimiter.push(b);
                    }
                    at += 1;
                }
                if delimiter.first().is_some_and(|b| !b.is_ascii_digit()) {
                    heredocs.push((delimiter, strip_tabs));
                }
            }
            b'\n' if !heredocs.is_empty() => {
                at += 1;
                for (delimiter, strip_tabs) in heredocs.drain(..) {
                    while at < bytes.len() {
                        let end = line_end(bytes, at);
                        let line = bytes.get(at..end).unwrap_or_default();
                        let line = if strip_tabs {
                            let tabs = line.iter().take_while(|&&b| b == b'\t').count();
                            line.get(tabs..).unwrap_or_default()
                        } else {
                            line
                        };
                        at = next_line(bytes, end);
                        if line == delimiter.as_slice() {
                            break;
                        }
                    }
                }
            }
            _ => at += 1,
        }
    }
    found
}
