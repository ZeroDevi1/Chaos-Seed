use std::borrow::Cow;

use crate::danmaku::model::DanmakuError;

// Tars/JCE type ids (see many public implementations).
const T_BYTE: u8 = 0;
const T_SHORT: u8 = 1;
const T_INT: u8 = 2;
const T_LONG: u8 = 3;
const T_STRING1: u8 = 6;
const T_STRING4: u8 = 7;
const T_LIST: u8 = 9;
const T_STRUCT_BEGIN: u8 = 10;
const T_STRUCT_END: u8 = 11;
const T_ZERO_TAG: u8 = 12;
const T_SIMPLE_LIST: u8 = 13;

pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    fn write_head(&mut self, tag: u8, ty: u8) {
        if tag < 15 {
            self.buf.push((tag << 4) | (ty & 0x0f));
        } else {
            self.buf.push(0xf0 | (ty & 0x0f));
            self.buf.push(tag);
        }
    }

    pub fn write_bool(&mut self, tag: u8, v: bool) {
        if !v {
            self.write_head(tag, T_ZERO_TAG);
            return;
        }
        self.write_head(tag, T_BYTE);
        self.buf.push(1);
    }

    pub fn write_i32(&mut self, tag: u8, v: i32) {
        if v == 0 {
            self.write_head(tag, T_ZERO_TAG);
            return;
        }
        self.write_head(tag, T_INT);
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_i64(&mut self, tag: u8, v: i64) {
        if v == 0 {
            self.write_head(tag, T_ZERO_TAG);
            return;
        }
        self.write_head(tag, T_LONG);
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_string(&mut self, tag: u8, s: &str) {
        let bytes = s.as_bytes();
        if bytes.len() < 255 {
            self.write_head(tag, T_STRING1);
            self.buf.push(bytes.len() as u8);
            self.buf.extend_from_slice(bytes);
        } else {
            self.write_head(tag, T_STRING4);
            let len: i32 = bytes.len().try_into().unwrap_or(i32::MAX);
            self.buf.extend_from_slice(&len.to_be_bytes());
            self.buf.extend_from_slice(bytes);
        }
    }

    pub fn write_bytes(&mut self, tag: u8, data: &[u8]) {
        // SIMPLE_LIST encoding for byte arrays:
        // head(tag, SIMPLE_LIST) + head(0, BYTE) [marker] + head(0, INT) + len + raw bytes
        self.write_head(tag, T_SIMPLE_LIST);
        self.write_head(0, T_BYTE); // marker only, no value

        let len: i32 = data.len().try_into().unwrap_or(i32::MAX);
        self.write_head(0, T_INT);
        self.buf.extend_from_slice(&len.to_be_bytes());
        self.buf.extend_from_slice(data);
    }
}

#[derive(Clone)]
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Result<u8, DanmakuError> {
        if self.pos >= self.buf.len() {
            return Err(DanmakuError::Codec("jce: unexpected eof".to_string()));
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_be_i16(&mut self) -> Result<i16, DanmakuError> {
        if self.remaining() < 2 {
            return Err(DanmakuError::Codec("jce: eof reading i16".to_string()));
        }
        let b = [self.buf[self.pos], self.buf[self.pos + 1]];
        self.pos += 2;
        Ok(i16::from_be_bytes(b))
    }

    fn read_be_i32(&mut self) -> Result<i32, DanmakuError> {
        if self.remaining() < 4 {
            return Err(DanmakuError::Codec("jce: eof reading i32".to_string()));
        }
        let b = [
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ];
        self.pos += 4;
        Ok(i32::from_be_bytes(b))
    }

    fn read_be_i64(&mut self) -> Result<i64, DanmakuError> {
        if self.remaining() < 8 {
            return Err(DanmakuError::Codec("jce: eof reading i64".to_string()));
        }
        let b = [
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
            self.buf[self.pos + 4],
            self.buf[self.pos + 5],
            self.buf[self.pos + 6],
            self.buf[self.pos + 7],
        ];
        self.pos += 8;
        Ok(i64::from_be_bytes(b))
    }

    fn read_head(&mut self) -> Result<(u8, u32), DanmakuError> {
        let b = self.read_u8()?;
        let ty = b & 0x0f;
        let mut tag = (b >> 4) as u32;
        if tag == 15 {
            tag = self.read_u8()? as u32;
        }
        Ok((ty, tag))
    }

    fn peek_head(&self) -> Result<(u8, u32, usize), DanmakuError> {
        if self.pos >= self.buf.len() {
            return Err(DanmakuError::Codec("jce: unexpected eof".to_string()));
        }
        let b = self.buf[self.pos];
        let ty = b & 0x0f;
        let mut tag = (b >> 4) as u32;
        let mut len = 1usize;
        if tag == 15 {
            if self.pos + 1 >= self.buf.len() {
                return Err(DanmakuError::Codec("jce: eof reading long tag".to_string()));
            }
            tag = self.buf[self.pos + 1] as u32;
            len = 2;
        }
        Ok((ty, tag, len))
    }

    fn skip(&mut self, n: usize) -> Result<(), DanmakuError> {
        if self.remaining() < n {
            return Err(DanmakuError::Codec("jce: skip out of range".to_string()));
        }
        self.pos += n;
        Ok(())
    }

    fn skip_to_struct_end(&mut self) -> Result<(), DanmakuError> {
        loop {
            let (ty, _tag) = self.read_head()?;
            if ty == T_STRUCT_END {
                return Ok(());
            }
            self.skip_field(ty)?;
        }
    }

    fn skip_field(&mut self, ty: u8) -> Result<(), DanmakuError> {
        match ty {
            T_ZERO_TAG | T_STRUCT_END => Ok(()),
            T_BYTE => self.skip(1),
            T_SHORT => self.skip(2),
            T_INT => self.skip(4),
            T_LONG => self.skip(8),
            T_STRING1 => {
                let len = self.read_u8()? as usize;
                self.skip(len)
            }
            T_STRING4 => {
                let len = self.read_be_i32()? as usize;
                self.skip(len)
            }
            T_LIST => {
                let (sty, _stag) = self.read_head()?; // size field head (tag=0)
                let size = self.read_int_by_type(sty)? as usize;
                for _ in 0..size {
                    let (ety, _etag) = self.read_head()?;
                    self.skip_field(ety)?;
                }
                Ok(())
            }
            T_SIMPLE_LIST => {
                // marker head(0, BYTE) without value
                let (mty, _mtag) = self.read_head()?;
                if mty != T_BYTE {
                    return Err(DanmakuError::Codec(
                        "jce: simple_list marker is not BYTE".to_string(),
                    ));
                }
                let (sty, _stag) = self.read_head()?; // size head
                let size = self.read_int_by_type(sty)? as usize;
                self.skip(size)
            }
            T_STRUCT_BEGIN => self.skip_to_struct_end(),
            other => Err(DanmakuError::Codec(format!(
                "jce: unsupported type {other}"
            ))),
        }
    }

    fn skip_to_tag(&mut self, target_tag: u32) -> Result<bool, DanmakuError> {
        loop {
            let (ty, tag, head_len) = match self.peek_head() {
                Ok(v) => v,
                Err(_) => return Ok(false),
            };
            if ty == T_STRUCT_END {
                return Ok(false);
            }
            if tag == target_tag {
                return Ok(true);
            }
            if tag > target_tag {
                return Ok(false);
            }
            // consume head
            self.skip(head_len)?;
            self.skip_field(ty)?;
        }
    }

    fn read_int_by_type(&mut self, ty: u8) -> Result<i64, DanmakuError> {
        match ty {
            T_ZERO_TAG => Ok(0),
            T_BYTE => Ok((self.read_u8()? as i8) as i64),
            T_SHORT => Ok(self.read_be_i16()? as i64),
            T_INT => Ok(self.read_be_i32()? as i64),
            T_LONG => Ok(self.read_be_i64()?),
            _ => Err(DanmakuError::Codec(
                "jce: type mismatch for int".to_string(),
            )),
        }
    }
}

pub fn get_i32(data: &[u8], tag: u32) -> Result<Option<i32>, DanmakuError> {
    let mut r = Reader::new(data);
    if !r.skip_to_tag(tag)? {
        return Ok(None);
    }
    let (ty, _tag) = r.read_head()?;
    let v = r.read_int_by_type(ty)?;
    Ok(Some(v as i32))
}

pub fn get_i64(data: &[u8], tag: u32) -> Result<Option<i64>, DanmakuError> {
    let mut r = Reader::new(data);
    if !r.skip_to_tag(tag)? {
        return Ok(None);
    }
    let (ty, _tag) = r.read_head()?;
    let v = r.read_int_by_type(ty)?;
    Ok(Some(v))
}

pub fn get_string(data: &[u8], tag: u32) -> Result<Option<String>, DanmakuError> {
    let mut r = Reader::new(data);
    if !r.skip_to_tag(tag)? {
        return Ok(None);
    }
    let (ty, _tag) = r.read_head()?;
    let bytes: Cow<'_, [u8]> = match ty {
        T_STRING1 => {
            let len = r.read_u8()? as usize;
            if r.remaining() < len {
                return Err(DanmakuError::Codec("jce: eof reading string1".to_string()));
            }
            let s = &r.buf[r.pos..r.pos + len];
            Cow::Borrowed(s)
        }
        T_STRING4 => {
            let len = r.read_be_i32()? as usize;
            if r.remaining() < len {
                return Err(DanmakuError::Codec("jce: eof reading string4".to_string()));
            }
            let s = &r.buf[r.pos..r.pos + len];
            Cow::Borrowed(s)
        }
        T_ZERO_TAG => return Ok(Some(String::new())),
        _ => {
            return Err(DanmakuError::Codec(
                "jce: type mismatch for string".to_string(),
            ));
        }
    };
    Ok(Some(String::from_utf8_lossy(&bytes).to_string()))
}

pub fn get_bytes(data: &[u8], tag: u32) -> Result<Option<Vec<u8>>, DanmakuError> {
    let mut r = Reader::new(data);
    if !r.skip_to_tag(tag)? {
        return Ok(None);
    }
    let (ty, _tag) = r.read_head()?;
    match ty {
        T_SIMPLE_LIST => {
            let (mty, _mtag) = r.read_head()?;
            if mty != T_BYTE {
                return Err(DanmakuError::Codec(
                    "jce: simple_list marker is not BYTE".to_string(),
                ));
            }
            let (sty, _stag) = r.read_head()?;
            let size = r.read_int_by_type(sty)? as usize;
            if r.remaining() < size {
                return Err(DanmakuError::Codec("jce: eof reading bytes".to_string()));
            }
            let out = r.buf[r.pos..r.pos + size].to_vec();
            Ok(Some(out))
        }
        T_LIST => {
            // list size as int field (tag=0)
            let (sty, _stag) = r.read_head()?;
            let size = r.read_int_by_type(sty)? as usize;
            let mut out = Vec::with_capacity(size);
            for _ in 0..size {
                let (ety, _etag) = r.read_head()?;
                let v = r.read_int_by_type(ety)?;
                out.push(v as u8);
            }
            Ok(Some(out))
        }
        T_ZERO_TAG => Ok(Some(Vec::new())),
        _ => Err(DanmakuError::Codec(
            "jce: type mismatch for bytes".to_string(),
        )),
    }
}

pub fn get_struct_bytes(data: &[u8], tag: u32) -> Result<Option<Vec<u8>>, DanmakuError> {
    let mut r = Reader::new(data);
    if !r.skip_to_tag(tag)? {
        return Ok(None);
    }
    let (ty, _tag) = r.read_head()?;
    if ty != T_STRUCT_BEGIN {
        return Err(DanmakuError::Codec(
            "jce: type mismatch for struct".to_string(),
        ));
    }
    let start = r.pos;
    loop {
        let (pty, _ptag, head_len) = r.peek_head()?;
        if pty == T_STRUCT_END {
            let end = r.pos;
            let _ = r.read_head()?; // consume STRUCT_END
            return Ok(Some(r.buf[start..end].to_vec()));
        }
        r.skip(head_len)?;
        r.skip_field(pty)?;
    }
}
