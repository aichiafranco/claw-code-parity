//! Parse copy into caption lines (plain text or SRT).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caption {
    pub text: String,
    pub start_us: Option<u64>,
    pub end_us: Option<u64>,
}

#[must_use]
pub fn parse_script(raw: &str) -> Vec<Caption> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if looks_like_srt(trimmed) {
        return parse_srt(trimmed);
    }
    split_plain(trimmed)
}

fn looks_like_srt(raw: &str) -> bool {
    raw.lines().any(|line| line.contains("-->"))
}

fn parse_srt(raw: &str) -> Vec<Caption> {
    let mut captions = Vec::new();
    let mut pending_time: Option<(u64, u64)> = None;
    let mut text_lines: Vec<String> = Vec::new();

    let flush = |captions: &mut Vec<Caption>,
                 pending_time: &mut Option<(u64, u64)>,
                 text_lines: &mut Vec<String>| {
        if text_lines.is_empty() {
            *pending_time = None;
            return;
        }
        let text = text_lines.join("").trim().to_string();
        text_lines.clear();
        if text.is_empty() {
            *pending_time = None;
            return;
        }
        let (start_us, end_us) = pending_time.take().unzip();
        captions.push(Caption {
            text,
            start_us,
            end_us,
        });
    };

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(&mut captions, &mut pending_time, &mut text_lines);
            continue;
        }
        if line.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        if let Some((start, end)) = parse_srt_times(line) {
            flush(&mut captions, &mut pending_time, &mut text_lines);
            pending_time = Some((start, end));
            continue;
        }
        text_lines.push(line.to_string());
    }
    flush(&mut captions, &mut pending_time, &mut text_lines);
    captions
}

fn parse_srt_times(line: &str) -> Option<(u64, u64)> {
    let (left, right) = line.split_once("-->")?;
    Some((
        parse_srt_clock(left.trim())?,
        parse_srt_clock(right.trim())?,
    ))
}

fn parse_srt_clock(value: &str) -> Option<u64> {
    let value = value.replace(',', ".");
    let (hms, frac) = value.split_once('.').unwrap_or((&value, "0"));
    let mut parts = hms.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let millis: u64 = frac
        .chars()
        .take(3)
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    Some(((hours * 3600 + minutes * 60 + seconds) * 1_000 + millis) * 1_000)
}

fn split_plain(raw: &str) -> Vec<Caption> {
    let paragraphs: Vec<String> = raw
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.replace('\n', ""))
        .collect();
    let mut captions = Vec::new();
    for paragraph in paragraphs {
        captions.extend(split_sentences(&paragraph).into_iter().map(|text| Caption {
            text,
            start_us: None,
            end_us: None,
        }));
    }
    captions
}

fn split_sentences(paragraph: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in paragraph.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '；' | '.' | '!' | '?' | ';') {
            let piece = current.trim().to_string();
            if !piece.is_empty() {
                sentences.push(piece);
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        sentences.push(tail.to_string());
    }
    if sentences.is_empty() {
        vec![paragraph.to_string()]
    } else {
        wrap_long_lines(sentences)
    }
}

fn wrap_long_lines(sentences: Vec<String>) -> Vec<String> {
    const MAX_CHARS: usize = 22;
    let mut wrapped = Vec::new();
    for sentence in sentences {
        let chars: Vec<char> = sentence.chars().collect();
        if chars.len() <= MAX_CHARS {
            wrapped.push(sentence);
            continue;
        }
        for chunk in chars.chunks(MAX_CHARS) {
            wrapped.push(chunk.iter().collect());
        }
    }
    wrapped
}

#[must_use]
pub fn character_weight(text: &str) -> usize {
    text.chars().filter(|ch| !ch.is_whitespace()).count().max(1)
}
