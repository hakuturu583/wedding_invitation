//! ZIP の読み書き(PDF のまとめと xlsx に使う).

/// ZIP を作る。compress が真なら Deflate で圧縮する(PDF は圧縮済みなので STORE で十分).
pub fn write(files: &[(&str, &[u8])], compress: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let crc = crc32fast::hash(data);
        let offset = out.len() as u32;
        let name = name.as_bytes();
        let (method, body) = if compress {
            (8u16, miniz_oxide::deflate::compress_to_vec(data, 6))
        } else {
            (0u16, data.to_vec())
        };
        let (csize, usize_) = (body.len() as u32, data.len() as u32);
        // ローカルファイルヘッダ: version, flags(UTF-8 名), method, time, date(1980-01-01)
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0x21, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&csize.to_le_bytes());
        out.extend_from_slice(&usize_.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&body);
        // セントラルディレクトリ: made by, needed, flags, method, time, date
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&[0, 0, 0x21, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&csize.to_le_bytes());
        central.extend_from_slice(&usize_.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 8]); // extra, comment, disk, internal attr
        central.extend_from_slice(&0u32.to_le_bytes()); // external attr
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn u16_at(d: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?) as usize)
}

fn u32_at(d: &[u8], i: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?) as usize)
}

/// 展開後の上限(壊れたファイルや圧縮爆弾でメモリを使い果たさないため).
const MAX_ENTRY_SIZE: usize = 64 * 1024 * 1024;

/// ZIP の全エントリを (名前, 中身) で返す。STORE と Deflate に対応.
pub fn read(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let bad = || "ZIP(xlsx)ファイルとして読み込めません".to_string();
    // 終端レコードを後ろから探す
    let eocd = (0..data.len().saturating_sub(21))
        .rev()
        .take(65557)
        .find(|&i| data[i..].starts_with(b"PK\x05\x06"))
        .ok_or_else(bad)?;
    let count = u16_at(data, eocd + 10).ok_or_else(bad)?;
    let mut pos = u32_at(data, eocd + 16).ok_or_else(bad)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if !data.get(pos..).is_some_and(|d| d.starts_with(b"PK\x01\x02")) {
            return Err(bad());
        }
        let method = u16_at(data, pos + 10).ok_or_else(bad)?;
        let csize = u32_at(data, pos + 20).ok_or_else(bad)?;
        let usize_ = u32_at(data, pos + 24).ok_or_else(bad)?;
        let name_len = u16_at(data, pos + 28).ok_or_else(bad)?;
        let extra_len = u16_at(data, pos + 30).ok_or_else(bad)?;
        let comment_len = u16_at(data, pos + 32).ok_or_else(bad)?;
        let local = u32_at(data, pos + 42).ok_or_else(bad)?;
        let name = data.get(pos + 46..pos + 46 + name_len).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(name).into_owned();
        pos += 46 + name_len + extra_len + comment_len;

        if !data.get(local..).is_some_and(|d| d.starts_with(b"PK\x03\x04")) {
            return Err(bad());
        }
        let start =
            local + 30 + u16_at(data, local + 26).ok_or_else(bad)? + u16_at(data, local + 28).ok_or_else(bad)?;
        let raw = data.get(start..start + csize).ok_or_else(bad)?;
        if usize_ > MAX_ENTRY_SIZE {
            return Err(format!("{name} が大きすぎます"));
        }
        let body = match method {
            0 => raw.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, MAX_ENTRY_SIZE).map_err(|_| bad())?,
            _ => return Err(format!("対応していない圧縮形式です({name})")),
        };
        entries.push((name, body));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let files: [(&str, &[u8]); 2] = [("a.txt", b"hello hello hello"), ("日本語/b.xml", b"<x/>")];
        for compress in [false, true] {
            let zip = write(&files, compress);
            let back = read(&zip).unwrap();
            assert_eq!(back.len(), 2);
            assert_eq!(back[0], ("a.txt".to_string(), b"hello hello hello".to_vec()));
            assert_eq!(back[1].0, "日本語/b.xml");
        }
        assert!(read(b"not a zip").is_err());
    }
}
