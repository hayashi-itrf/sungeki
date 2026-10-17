"""Measured subtitle wrapping. Keep source text/explicit empty lines intact."""
import unicodedata

NO_START = set('、。，．？！!?：；)]}）］｝」』】〉》ーぁぃぅぇぉっゃゅょァィゥェォッャュョ')
NO_END = set('([{（［｛「『【〈《')


def word_kind(char):
    if char.isascii() and char.isalnum():
        return 'ascii'
    if '\u30a1' <= char <= '\u30fa' or char == 'ー' or '\uff66' <= char <= '\uff9d':
        return 'katakana'
    return None


def cuts_word(before, after):
    kind = word_kind(before)
    return kind is not None and kind == word_kind(after)


def units(text):
    result = []
    regional = 0
    for char in text:
        cp = ord(char)
        attached = (unicodedata.category(char).startswith('M') or char == '\u200d'
                    or 0xfe00 <= cp <= 0xfe0f or 0x1f3fb <= cp <= 0x1f3ff
                    or cp in (0xff9e, 0xff9f))
        ri = 0x1f1e6 <= cp <= 0x1f1ff
        if result and (attached or result[-1].endswith('\u200d') or (ri and regional % 2 == 1)):
            result[-1] += char
        else:
            result.append(char)
        regional = regional + 1 if ri else 0
    return result


def balance_tail(output, measure, width):
    """Reflow an orphaned CJK tail without adding lines or touching explicit breaks."""
    if len(output) < 2:
        return output
    left, right = output[-2:]
    if measure(right) >= width * .35 or not any(
            unicodedata.east_asian_width(c) in ('W', 'F') for c in left + right):
        return output
    parts = units(left + right)
    # Bound expensive font measurements; long paragraphs keep greedy wrapping.
    if len(parts) > 128:
        return output
    best = abs(measure(left) - measure(right))
    pair = (left, right)
    for split in range(1, len(parts)):
        before, after = parts[split - 1][-1], parts[split][0]
        if after in NO_START or before in NO_END:
            continue
        if cuts_word(parts[split - 1][0], after):
            continue
        a, b = ''.join(parts[:split]), ''.join(parts[split:])
        wa, wb = measure(a), measure(b)
        if max(wa, wb) > width:
            continue
        # A sentence/clause boundary is preferable to a slightly more even split.
        bonus = .2 if before in '。、！？!?;' else .15 if before in 'はがをにでとへ' else 0
        score = abs(wa - wb) - width * bonus
        if score < best:
            best, pair = score, (a, b)
    return output[:-2] + list(pair)


def lines(text, measure, width, wrap):
    output = []
    # Match text-area line breaks without rewriting the stored Project text.
    for paragraph in text.replace('\r\n', '\n').replace('\r', '\n').split('\n'):
        if not wrap or not paragraph:
            output.append(paragraph)
            continue
        paragraph_lines = []
        parts = units(paragraph)
        offset = 0
        while offset < len(parts):
            # Binary search bounds costly font measurements for long input.
            low, high = 1, len(parts) - offset
            best = 0
            while low <= high:
                mid = (low + high) // 2
                if measure(''.join(parts[offset:offset + mid])) <= width:
                    best = mid
                    low = mid + 1
                else:
                    high = mid - 1
            count = max(1, best)  # Too-wide glyph stays intact and is reported as overflow.
            if offset + count < len(parts):
                candidate = count
                while candidate > 1 and (parts[offset + candidate][0] in NO_START
                        or parts[offset + candidate - 1][-1] in NO_END):
                    candidate -= 1
                count = candidate
                # Keep ASCII and katakana runs together when a prior legal
                # boundary fits. Overlong runs still split to avoid overflow.
                if cuts_word(parts[offset + count - 1][0], parts[offset + count][0]):
                    for i in range(count - 1, 0, -1):
                        before, after = parts[offset + i - 1][0], parts[offset + i][0]
                        if not cuts_word(before, after) and after not in NO_START and before not in NO_END:
                            count = i
                            break
            paragraph_lines.append(''.join(parts[offset:offset + count]))
            offset += count
        output.extend(balance_tail(paragraph_lines, measure, width))
    return output
