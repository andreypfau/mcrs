use crate::packet_io::ByteStream;
use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use js_sys::{Reflect, Uint8Array};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ReadableStreamDefaultReader, WebTransport, WebTransportHash, WebTransportOptions,
    WritableStreamDefaultWriter,
};

/// Where the browser dials, and the SHA-256 of the certificate the server
/// minted at startup: a self-signed endpoint is unreachable without it, and the
/// server publishes no discovery endpoint, so it travels in the page URL.
#[derive(Clone, Debug)]
pub struct WebTransportTarget {
    pub url: String,
    pub certificate_hash: [u8; 32],
}

impl WebTransportTarget {
    /// The host and port the handshake packet announces, which a proxy in front
    /// of the server reads and a vanilla server ignores.
    pub fn host_and_port(&self) -> (String, u16) {
        match web_sys::Url::new(&self.url) {
            Ok(url) => {
                let port = url.port().parse().unwrap_or(443);
                (url.hostname(), port)
            }
            Err(_) => (self.url.clone(), 443),
        }
    }
}

/// Reads `?server=https://host:port/path&cert=<64 hex digits>`.
pub fn target_from_query(url: &str, certificate_hash: &str) -> anyhow::Result<WebTransportTarget> {
    Ok(WebTransportTarget {
        url: url.to_owned(),
        certificate_hash: crate::certificate_hash_from_hex(certificate_hash)?,
    })
}

pub async fn connect(target: &WebTransportTarget) -> anyhow::Result<BrowserStream> {
    let hash = WebTransportHash::new();
    hash.set_algorithm("sha-256");
    hash.set_value(Uint8Array::from(&target.certificate_hash[..]).unchecked_ref());

    let options = WebTransportOptions::new();
    options.set_server_certificate_hashes(&[hash]);

    let transport = WebTransport::new_with_options(&target.url, &options).map_err(js_error)?;
    JsFuture::from(transport.ready())
        .await
        .map_err(|e| anyhow!("WebTransport to {} never became ready: {}", target.url, js(e)))?;

    let stream = JsFuture::from(transport.create_bidirectional_stream())
        .await
        .map_err(js_error)?;

    let reader = ReadableStreamDefaultReader::new(&stream.readable()).map_err(js_error)?;
    let writer = WritableStreamDefaultWriter::new(&stream.writable()).map_err(js_error)?;
    Ok(BrowserStream {
        reader: StreamReader {
            reader,
            pending: None,
            leftover: Bytes::new(),
            done: false,
            // The session object owns the streams; dropping it closes them.
            _transport: transport.clone(),
        },
        writer: StreamWriter {
            writer,
            pending: None,
            _transport: transport,
        },
    })
}

pub struct BrowserStream {
    reader: StreamReader,
    writer: StreamWriter,
}

impl ByteStream for BrowserStream {
    type Reader = StreamReader;
    type Writer = StreamWriter;

    fn split_stream(self) -> (Self::Reader, Self::Writer) {
        (self.reader, self.writer)
    }
}

impl AsyncRead for BrowserStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl AsyncWrite for BrowserStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.writer).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.writer).poll_shutdown(cx)
    }
}

pub struct StreamReader {
    reader: ReadableStreamDefaultReader,
    pending: Option<JsFuture>,
    leftover: Bytes,
    done: bool,
    _transport: WebTransport,
}

impl AsyncRead for StreamReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            if !this.leftover.is_empty() {
                let take = this.leftover.len().min(buf.remaining());
                buf.put_slice(&this.leftover.split_to(take));
                return Poll::Ready(Ok(()));
            }
            if this.done {
                return Poll::Ready(Ok(()));
            }
            let pending = this
                .pending
                .get_or_insert_with(|| JsFuture::from(this.reader.read()));
            let result = ready!(Pin::new(pending).poll(cx));
            this.pending = None;
            let result = result.map_err(io_error)?;
            if Reflect::get(&result, &JsValue::from_str("done"))
                .map_err(io_error)?
                .is_truthy()
            {
                this.done = true;
                return Poll::Ready(Ok(()));
            }
            let chunk: Uint8Array = Reflect::get(&result, &JsValue::from_str("value"))
                .map_err(io_error)?
                .unchecked_into();
            let mut bytes = BytesMut::zeroed(chunk.length() as usize);
            chunk.copy_to(&mut bytes);
            this.leftover = bytes.freeze();
        }
    }
}

pub struct StreamWriter {
    writer: WritableStreamDefaultWriter,
    pending: Option<JsFuture>,
    _transport: WebTransport,
}

impl StreamWriter {
    fn poll_pending(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let Some(pending) = self.pending.as_mut() else {
            return Poll::Ready(Ok(()));
        };
        let result = ready!(Pin::new(pending).poll(cx));
        self.pending = None;
        Poll::Ready(result.map(|_| ()).map_err(io_error))
    }
}

impl AsyncWrite for StreamWriter {
    /// One write is in flight at a time: the next one waits for the promise the
    /// previous returned, which is where the stream's backpressure lands.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        ready!(this.poll_pending(cx))?;
        let chunk = JsValue::from(Uint8Array::from(buf));
        this.pending = Some(JsFuture::from(this.writer.write_with_chunk(&chunk)));
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().poll_pending(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        ready!(this.poll_pending(cx))?;
        this.pending = Some(JsFuture::from(this.writer.close()));
        this.poll_pending(cx)
    }
}

fn js(value: JsValue) -> String {
    format!("{value:?}")
}

fn js_error(value: JsValue) -> anyhow::Error {
    anyhow!("{}", js(value))
}

fn io_error(value: JsValue) -> io::Error {
    io::Error::other(js(value))
}
