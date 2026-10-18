//! Measured wrapping shared by native exports and text diagnostics.
use unicode_segmentation::UnicodeSegmentation;
const NO_START: &str =
    "、。，．？！!?：；)]}）］｝」』】〉》ーぁぃぅぇぉっゃゅょァィゥェォッャュョ";
const NO_END: &str = "([{（［｛「『【〈《";
fn kind(c: char) -> u8 {
    if c.is_ascii_alphanumeric() {
        1
    } else if ('ァ'..='ヺ').contains(&c) || c == 'ー' || ('ｦ'..='ﾝ').contains(&c) {
        2
    } else {
        0
    }
}
fn cuts(a: &str, b: &str) -> bool {
    let x = kind(a.chars().next().unwrap());
    x != 0 && x == kind(b.chars().next().unwrap())
}
fn legal(a: &str, b: &str) -> bool {
    !NO_END.contains(a.chars().last().unwrap()) && !NO_START.contains(b.chars().next().unwrap())
}
pub(crate) fn lines(
    text: &str,
    width: f64,
    wrap: bool,
    measure: impl Fn(&str) -> f64,
) -> Vec<String> {
    let mut output = vec![];
    for paragraph in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if !wrap || paragraph.is_empty() {
            output.push(paragraph.into());
            continue;
        }
        let parts: Vec<_> = paragraph.graphemes(true).collect();
        let mut offset = 0;
        let mut rows = vec![];
        while offset < parts.len() {
            let (mut lo, mut hi, mut best) = (1, parts.len() - offset, 0);
            while lo <= hi {
                let mid = (lo + hi) / 2;
                if measure(&parts[offset..offset + mid].concat()) <= width {
                    best = mid;
                    lo = mid + 1;
                } else {
                    hi = mid - 1;
                }
            }
            let mut n = best.max(1);
            if offset + n < parts.len() {
                while n > 1 && !legal(parts[offset + n - 1], parts[offset + n]) {
                    n -= 1;
                }
                if cuts(parts[offset + n - 1], parts[offset + n]) {
                    for i in (1..n).rev() {
                        if !cuts(parts[offset + i - 1], parts[offset + i])
                            && legal(parts[offset + i - 1], parts[offset + i])
                        {
                            n = i;
                            break;
                        }
                    }
                }
            }
            rows.push(parts[offset..offset + n].concat());
            offset += n;
        }
        balance(&mut rows, width, &measure);
        output.extend(rows);
    }
    output
}
fn balance(rows: &mut [String], width: f64, measure: &impl Fn(&str) -> f64) {
    let n = rows.len();
    if n < 2 || measure(&rows[n - 1]) >= width * 0.35 {
        return;
    }
    let joined = format!("{}{}", rows[n - 2], rows[n - 1]);
    if !joined.chars().any(|c| c >= '\u{3000}') {
        return;
    }
    let p: Vec<_> = joined.graphemes(true).collect();
    if p.len() > 128 {
        return;
    }
    let mut best = (measure(&rows[n - 2]) - measure(&rows[n - 1])).abs();
    for i in 1..p.len() {
        if !legal(p[i - 1], p[i]) || cuts(p[i - 1], p[i]) {
            continue;
        }
        let (a, b) = (p[..i].concat(), p[i..].concat());
        let (wa, wb) = (measure(&a), measure(&b));
        if wa.max(wb) > width {
            continue;
        }
        let before = p[i - 1].chars().last().unwrap();
        let bonus = if "。、！？!?;".contains(before) {
            0.2
        } else if "はがをにでとへ".contains(before) {
            0.15
        } else {
            0.
        };
        let score = (wa - wb).abs() - width * bonus;
        if score < best {
            best = score;
            rows[n - 2] = a;
            rows[n - 1] = b;
        }
    }
}
