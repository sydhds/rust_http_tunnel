use tokio_util::codec::{Decoder, Encoder};
use bytes::BytesMut;

// traits
use std::fmt::Write; // for write_fmt()

// Note: here we define a 'codec' that will handle the http thingies
//
// The following traits need to be implemented (see tokio util doc)
// codec.decode() -> Parse bytes (HTTP Connect request)
// codec.encode() -> Send HTTP response (usually 200 OK)

#[derive(Debug)]
pub struct HttpCodec {
}

const MAX_HTTP_CONNECT_SIZE: usize = 1024; // enough for "CONNECT ..."
const HTTP_CONNECT_START: &[u8] = b"CONNECT ";
const HTTP_CONNECT_END: &[u8] = b" HTTP/1.1";
const HTTP_CONNECT_SLICE_START: usize = HTTP_CONNECT_START.len();
const HTTP_CONNECT_SLICE_END_SIZE: usize = HTTP_CONNECT_END.len();

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("i/o error: {0}")]
    IO(#[from] std::io::Error),
    #[error("utf8 error: {0}")]
    UTF8(#[from] std::string::FromUtf8Error)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

impl Decoder for HttpCodec {

    // Decoder will take some bytes, parse them and extract the destination url
    // e.g. "CONNECT URL:PORT HTTP/1.1\r\n" -> "URL:PORT"

    type Item = String;
    type Error = DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {

        if !src.ends_with(b"\r\n") {
            return Ok(None); // not enough data
        }

        if src.len() >= MAX_HTTP_CONNECT_SIZE {
            return Err(DecodeError::IO(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                           format!("HTTP frame too large: {}", src.len()))));
        }

        if !src.starts_with(HTTP_CONNECT_START) {
            return Err(DecodeError::IO(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                           format!("Invalid HTTP request"))));
        }

        let http_connect_end_index = find_subsequence(src, b" HTTP/1.1\r\n");

        if http_connect_end_index.is_none() {
            return Err(DecodeError::IO(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                           format!("Invalid HTTP request"))));
        }

        // unwrap is safe here
        let url_ : &[u8] = &src[HTTP_CONNECT_SLICE_START..http_connect_end_index.unwrap()];
        let url: String = String::from_utf8(url_.to_vec())?;
        Ok(Some(url))
    }

}

#[repr(u32)]
pub enum TunnelResult {
    Ok, // 200
    BadRequest, // 400
    Forbidden, // 403
    Timeout, // 408
    ServerError, // 500
}

impl Encoder<TunnelResult> for HttpCodec {

    type Error = std::io::Error;

    fn encode(&mut self, tunnel_result: TunnelResult, dst: &mut BytesMut) -> Result<(), Self::Error> {

        let (code, message): (u32, &str) = match tunnel_result {
            TunnelResult::Ok => (200, "OK"),
            TunnelResult::BadRequest => (400, "BAD_REQUEST"),
            TunnelResult::Forbidden => (408, "Timeout"),
            TunnelResult::ServerError => (500, "SERVER_ERROR"),
            _ => (400, "BAD_REQUEST"),
        };

        dst.write_fmt(format_args!("HTTP/1.1 {} {}\r\n\r\n", code, message)).map_err(
            |_| std::io::Error::from(std::io::ErrorKind::Other)
        )
    }
}

#[cfg(test)]
mod tests {

    use super::{HttpCodec, DecodeError};

    // traits
    use tokio_util::codec::{Encoder, Decoder}; // for encode() / decode()
    use bytes::BufMut;
    use crate::codec::TunnelResult; // for put()

    #[test]
    fn test_decode_valid_request() -> Result<(), DecodeError> {
        let http_req = b"CONNECT google.com:80 HTTP/1.1\r\n";
        let mut codec = HttpCodec {};
        let mut buffer = bytes::BytesMut::with_capacity(http_req.len());
        buffer.put(&http_req[..]);

        assert_eq!(codec.decode(&mut buffer)?.unwrap(), "google.com:80");
        Ok(())
    }

    #[test]
    fn test_decode_valid_request_2() -> Result<(), DecodeError> {
        let http_req = b"CONNECT google.com:80 HTTP/1.1\r\nHost: google.com:80\r\nUser-Agent: curl/7.64.0\r\nProxy-Connection: Keep-Alive\r\n\r\n";
        let mut codec = HttpCodec {};
        let mut buffer = bytes::BytesMut::with_capacity(http_req.len());
        buffer.put(&http_req[..]);

        assert_eq!(codec.decode(&mut buffer)?.unwrap(), "google.com:80");
        Ok(())
    }

    #[test]
    fn test_decode_invalid_request() {

        let http_req = b"CONNEC google.com:80 HTTP/1.1\r\n"; // CONNEC vs CONNECT
        let mut codec = HttpCodec {};
        let mut buffer = bytes::BytesMut::with_capacity(http_req.len());
        buffer.put(&http_req[..]);

        if let Err(DecodeError::IO(e)) = codec.decode(&mut buffer) {
            assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        }
        else {
            panic!("Shoud not happen")
        }
    }

    #[test]
    fn test_decode_large_request() {

        let http_req = b"CONNECT {}:80 HTTP/1.1\r\n";
        let mut codec = HttpCodec {};
        let mut buffer = bytes::BytesMut::new();
        for i in 0..48 {
            buffer.put(&http_req[..]);
        }

        if let Err(DecodeError::IO(e)) = codec.decode(&mut buffer) {
            assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        } else {
            panic!("Should not happen")
        }
    }

    #[test]
    fn test_encode_200() -> Result<(), std::io::Error> {
        let mut codec = HttpCodec {};
        let mut buffer = bytes::BytesMut::new();
        codec.encode(TunnelResult::Ok, &mut buffer)?;
        assert_eq!(buffer, b"HTTP/1.1 200 OK\r\n\r\n"[..]);
        Ok(())
    }
}
