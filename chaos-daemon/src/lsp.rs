use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

#[derive(Debug, Error)]
pub enum LspFrameError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing Content-Length header")]
    MissingContentLength,
    #[error("invalid Content-Length header")]
    InvalidContentLength,
    #[error("payload too large: {0}")]
    PayloadTooLarge(usize),
}

pub async fn read_lsp_frame<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    max_len: usize,
) -> Result<Vec<u8>, LspFrameError> {
    let mut content_len: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
        }

        if line == "\r\n" {
            break;
        }

        if let Some(v) = line.strip_prefix("Content-Length:") {
            let v = v.trim();
            let len: usize = v.parse().map_err(|_| LspFrameError::InvalidContentLength)?;
            content_len = Some(len);
        }
    }

    let len = content_len.ok_or(LspFrameError::MissingContentLength)?;
    if len > max_len {
        return Err(LspFrameError::PayloadTooLarge(len));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(buf)
}

pub async fn write_lsp_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<(), LspFrameError> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_lsp_frame, write_lsp_frame};
    use tokio::io::AsyncWriteExt;
    use tokio::io::{BufReader, duplex};

    #[tokio::test]
    async fn parses_frame_when_split() {
        let (mut a, b) = duplex(1024);
        let mut br = BufReader::new(b);

        tokio::spawn(async move {
            a.write_all(b"Content-Length: 2\r\n").await.unwrap();
            a.write_all(b"\r\n").await.unwrap();
            a.write_all(b"{}").await.unwrap();
        });

        let got = read_lsp_frame(&mut br, 1024).await.unwrap();
        assert_eq!(got, b"{}");
    }

    #[tokio::test]
    async fn parses_multiple_frames_back_to_back() {
        let (mut a, b) = duplex(2048);
        let mut br = BufReader::new(b);

        tokio::spawn(async move {
            write_lsp_frame(&mut a, b"{\"a\":1}").await.unwrap();
            write_lsp_frame(&mut a, b"{\"b\":2}").await.unwrap();
        });

        let f1 = read_lsp_frame(&mut br, 1024).await.unwrap();
        let f2 = read_lsp_frame(&mut br, 1024).await.unwrap();
        assert!(std::str::from_utf8(&f1).unwrap().contains("\"a\""));
        assert!(std::str::from_utf8(&f2).unwrap().contains("\"b\""));
    }
}
