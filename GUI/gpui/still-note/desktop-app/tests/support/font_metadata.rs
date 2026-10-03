//! Inspect the bundled binaries independently of the product registration code.
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
    assert_eq!(&font[..4], b"OTTO", "official static Pretendard asset must be OpenType");
    for record in (12..12 + usize::from(u16_at(font, 4)) * 16).step_by(16) {
        if &font[record..record + 4] == tag {
            let offset = u32_at(font, record + 8) as usize;
            let len = u32_at(font, record + 12) as usize;
            return &font[offset..offset + len];
        }
    }
    panic!("missing font table {}", String::from_utf8_lossy(tag));
}
fn has_pretendard_family(font: &[u8]) -> bool {
    let names = table(font, b"name");
    let storage = usize::from(u16_at(names, 4));
    (6..6 + usize::from(u16_at(names, 2)) * 12).step_by(12).any(|record| {
        let platform = u16_at(names, record);
        let id = u16_at(names, record + 6);
        if !matches!(platform, 0 | 3) || !matches!(id, 1 | 16) {
            return false;
        }
        let start = storage + usize::from(u16_at(names, record + 10));
        let end = start + usize::from(u16_at(names, record + 8));
        let words: Vec<_> = names[start..end].as_chunks::<2>().0.iter().map(|word| u16_at(word, 0)).collect();
        String::from_utf16(&words).unwrap().contains("Pretendard")
    })
}
fn mapped_glyph(cmap: &[u8], ch: char) -> bool {
    let code = ch as u32;
    (4..4 + usize::from(u16_at(cmap, 2)) * 8).step_by(8).any(|record| {
        let sub = &cmap[u32_at(cmap, record + 4) as usize..];
        match u16_at(sub, 0) {
            | 12 => (16..16 + u32_at(sub, 12) as usize * 12).step_by(12).any(|group| {
                let start = u32_at(sub, group);
                let end = u32_at(sub, group + 4);
                code >= start && code <= end && u32_at(sub, group + 8) + code - start != 0
            }),
            | 4 if code <= 0xffff => {
                let segments = usize::from(u16_at(sub, 6)) / 2;
                (0..segments).any(|index| {
                    let end = u16_at(sub, 14 + index * 2);
                    let start = u16_at(sub, 16 + segments * 2 + index * 2);
                    if code < u32::from(start) || code > u32::from(end) {
                        return false;
                    }
                    let delta = u16_at(sub, 16 + segments * 4 + index * 2);
                    let range_at = 16 + segments * 6 + index * 2;
                    let range = u16_at(sub, range_at);
                    let glyph = if range == 0 {
                        (code as u16).wrapping_add(delta)
                    } else {
                        let raw = u16_at(sub, range_at + usize::from(range) + (code as usize - usize::from(start)) * 2);
                        if raw == 0 { 0 } else { raw.wrapping_add(delta) }
                    };
                    glyph != 0
                })
            },
            | _ => false,
        }
    })
}

#[test]
fn font_ac01_ac02_bundled_pretendard_real_metadata_weights_and_full_hangul() {
    let fonts: [(u16, &[u8]); 4] = [
        (400, include_bytes!("../../assets/fonts/Pretendard-Regular.otf")),
        (500, include_bytes!("../../assets/fonts/Pretendard-Medium.otf")),
        (600, include_bytes!("../../assets/fonts/Pretendard-SemiBold.otf")),
        (700, include_bytes!("../../assets/fonts/Pretendard-Bold.otf")),
    ];
    for (expected_weight, bytes) in fonts {
        assert!(has_pretendard_family(bytes), "binary family must be Pretendard at {expected_weight}");
        assert_eq!(u16_at(table(bytes, b"OS/2"), 4), expected_weight);
        let cmap = table(bytes, b"cmap");
        for code in 0xac00..=0xd7a3 {
            assert!(
                mapped_glyph(cmap, char::from_u32(code).unwrap()),
                "missing Hangul U+{code:X}, weight {expected_weight}"
            );
        }
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789한글기록검색".chars() {
            assert!(mapped_glyph(cmap, ch), "missing UI character {ch}, weight {expected_weight}");
        }
    }
    let license = include_str!("../../assets/fonts/LICENSE-Pretendard.txt");
    assert!(license.contains("SIL OPEN FONT LICENSE"));
}
