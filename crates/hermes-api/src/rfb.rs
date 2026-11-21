//! A small RFB (VNC, RFC 6143) client for the Bot Screen.
//!
//! The Bot Screen bridge `/api/display/ws?display_ticket=..` carries raw RFB bytes in binary
//! WebSocket frames (hermes-agent `hermes_cli/web_routers/display.py`). Xvnc runs with
//! `-SecurityTypes None`, so the handshake is: version, security type 1, ClientInit.
//!
//! Supported encodings: Raw (0), CopyRect (1), ZRLE (16) and the DesktopSize pseudo-encoding (-223).
//! The client asks for 32 bpp, depth 24, little-endian true colour, so ZRLE CPIXELs are 3 bytes.

use flate2::{Decompress, FlushDecompress};
use futures::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

/// Largest framebuffer side we accept (bounded memory).
pub const MAX_SIDE: u16 = 8192;

pub const ENC_RAW: i32 = 0;
pub const ENC_COPYRECT: i32 = 1;
pub const ENC_ZRLE: i32 = 16;
pub const ENC_DESKTOP_SIZE: i32 = -223;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum RfbError {
    #[error("The Bot Screen stream ended")]
    Eof,
    #[error("The Bot Screen sent data this app cannot read: {0}")]
    Protocol(String),
    #[error("The Bot Screen needs a password type this app does not support")]
    Security,
}

/// Pulls bytes out of a stream of binary chunks (one chunk per WebSocket frame).
pub struct ChunkReader {
    rx: mpsc::Receiver<Vec<u8>>,
    buf: VecDeque<u8>,
}

impl ChunkReader {
    pub fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self { rx, buf: VecDeque::new() }
    }

    pub async fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, RfbError> {
        while self.buf.len() < n {
            match self.rx.recv().await {
                Some(chunk) => self.buf.extend(chunk),
                None => return Err(RfbError::Eof),
            }
        }
        Ok(self.buf.drain(..n).collect())
    }

    pub async fn u8(&mut self) -> Result<u8, RfbError> {
        Ok(self.read_exact(1).await?[0])
    }
    pub async fn u16(&mut self) -> Result<u16, RfbError> {
        let b = self.read_exact(2).await?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub async fn u32(&mut self) -> Result<u32, RfbError> {
        let b = self.read_exact(4).await?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub async fn i32(&mut self) -> Result<i32, RfbError> {
        Ok(self.u32().await? as i32)
    }
}

/// An RGBA framebuffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Framebuffer {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u16, height: u16) -> Self {
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Self { width, height, rgba }
    }

    fn in_bounds(&self, x: u16, y: u16, w: u16, h: u16) -> bool {
        x as u32 + w as u32 <= self.width as u32 && y as u32 + h as u32 <= self.height as u32
    }

    /// Write one pixel given as server-format little-endian B,G,R bytes.
    #[inline]
    fn put_bgr(&mut self, x: usize, y: usize, b: u8, g: u8, r: u8) {
        let i = (y * self.width as usize + x) * 4;
        self.rgba[i] = r;
        self.rgba[i + 1] = g;
        self.rgba[i + 2] = b;
        self.rgba[i + 3] = 255;
    }

    fn copy_rect(&mut self, sx: u16, sy: u16, dx: u16, dy: u16, w: u16, h: u16) {
        let stride = self.width as usize * 4;
        let row_len = w as usize * 4;
        let mut tmp = vec![0u8; row_len * h as usize];
        for row in 0..h as usize {
            let s = (sy as usize + row) * stride + sx as usize * 4;
            tmp[row * row_len..(row + 1) * row_len].copy_from_slice(&self.rgba[s..s + row_len]);
        }
        for row in 0..h as usize {
            let d = (dy as usize + row) * stride + dx as usize * 4;
            self.rgba[d..d + row_len].copy_from_slice(&tmp[row * row_len..(row + 1) * row_len]);
        }
    }
}

// ---- client -> server messages (pure encoders) ----

/// SetPixelFormat: 32 bpp, depth 24, little-endian, true colour, R<<16 G<<8 B.
pub fn set_pixel_format() -> Vec<u8> {
    let mut m = vec![0u8, 0, 0, 0];
    m.extend_from_slice(&[32, 24, 0, 1]); // bpp, depth, big-endian=0, true-colour=1
    m.extend_from_slice(&255u16.to_be_bytes());
    m.extend_from_slice(&255u16.to_be_bytes());
    m.extend_from_slice(&255u16.to_be_bytes());
    m.extend_from_slice(&[16, 8, 0, 0, 0, 0]);
    m
}

pub fn set_encodings(encs: &[i32]) -> Vec<u8> {
    let mut m = vec![2u8, 0];
    m.extend_from_slice(&(encs.len() as u16).to_be_bytes());
    for e in encs {
        m.extend_from_slice(&e.to_be_bytes());
    }
    m
}

pub fn fb_update_request(incremental: bool, x: u16, y: u16, w: u16, h: u16) -> Vec<u8> {
    let mut m = vec![3u8, incremental as u8];
    for v in [x, y, w, h] {
        m.extend_from_slice(&v.to_be_bytes());
    }
    m
}

pub fn key_event(down: bool, keysym: u32) -> Vec<u8> {
    let mut m = vec![4u8, down as u8, 0, 0];
    m.extend_from_slice(&keysym.to_be_bytes());
    m
}

pub fn pointer_event(buttons: u8, x: u16, y: u16) -> Vec<u8> {
    let mut m = vec![5u8, buttons];
    m.extend_from_slice(&x.to_be_bytes());
    m.extend_from_slice(&y.to_be_bytes());
    m
}

// ---- protocol driver ----

/// Output of the decoder for the UI.
#[derive(Debug, Clone)]
pub enum RfbEvent {
    /// Connected; the server name and size.
    Ready { name: String, width: u16, height: u16 },
    /// A new full frame (shared, so cloning is cheap).
    Frame(Arc<Framebuffer>),
    /// The stream ended. `code` is the WebSocket close code (4000 control-taken, 4001 screen gone, 4401 bad ticket).
    Closed { code: Option<u16>, reason: String },
}

/// The handshake for `SecurityTypes None`. Returns (name, width, height).
pub async fn handshake(r: &mut ChunkReader, send: &mpsc::Sender<Vec<u8>>) -> Result<(String, u16, u16), RfbError> {
    let version = r.read_exact(12).await?;
    if !version.starts_with(b"RFB ") {
        return Err(RfbError::Protocol("bad version banner".into()));
    }
    // Answer with 3.8 (Xvnc supports it).
    send.send(b"RFB 003.008\n".to_vec()).await.map_err(|_| RfbError::Eof)?;
    let n = r.u8().await?;
    if n == 0 {
        let len = r.u32().await?.min(1024) as usize;
        let reason = String::from_utf8_lossy(&r.read_exact(len).await?).into_owned();
        return Err(RfbError::Protocol(reason));
    }
    let types = r.read_exact(n as usize).await?;
    if !types.contains(&1) {
        return Err(RfbError::Security);
    }
    send.send(vec![1]).await.map_err(|_| RfbError::Eof)?;
    let result = r.u32().await?;
    if result != 0 {
        return Err(RfbError::Security);
    }
    // ClientInit: shared = 1 (other viewers stay connected).
    send.send(vec![1]).await.map_err(|_| RfbError::Eof)?;
    let width = r.u16().await?;
    let height = r.u16().await?;
    let _pixel_format = r.read_exact(16).await?;
    let name_len = r.u32().await?.min(4096) as usize;
    let name = String::from_utf8_lossy(&r.read_exact(name_len).await?).into_owned();
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(RfbError::Protocol(format!("bad screen size {width}x{height}")));
    }
    Ok((name, width, height))
}

/// Decoder state kept across messages (the ZRLE zlib stream is persistent).
pub struct Decoder {
    pub fb: Framebuffer,
    zlib: Decompress,
}

impl Decoder {
    pub fn new(width: u16, height: u16) -> Self {
        Self { fb: Framebuffer::new(width, height), zlib: Decompress::new(true) }
    }

    /// Read one server message. Returns `true` if it was a framebuffer update.
    pub async fn read_message(&mut self, r: &mut ChunkReader) -> Result<bool, RfbError> {
        let kind = r.u8().await?;
        match kind {
            0 => {
                let _pad = r.u8().await?;
                let n = r.u16().await?;
                for _ in 0..n {
                    let x = r.u16().await?;
                    let y = r.u16().await?;
                    let w = r.u16().await?;
                    let h = r.u16().await?;
                    let enc = r.i32().await?;
                    self.rect(r, x, y, w, h, enc).await?;
                }
                Ok(true)
            }
            1 => {
                // SetColourMapEntries: skip (we use true colour).
                let _ = r.read_exact(3).await?;
                let n = r.u16().await? as usize;
                r.read_exact(n * 6).await?;
                Ok(false)
            }
            2 => Ok(false), // Bell
            3 => {
                let _ = r.read_exact(3).await?;
                let len = r.u32().await? as usize;
                if len > 1024 * 1024 {
                    return Err(RfbError::Protocol("cut text too long".into()));
                }
                r.read_exact(len).await?;
                Ok(false)
            }
            150 => Ok(false), // EndOfContinuousUpdates
            248 => {
                // Fence: 3 pad, u32 flags, u8 len, payload.
                let _ = r.read_exact(7).await?;
                let len = r.u8().await? as usize;
                r.read_exact(len).await?;
                Ok(false)
            }
            other => Err(RfbError::Protocol(format!("unknown server message {other}"))),
        }
    }

    async fn rect(&mut self, r: &mut ChunkReader, x: u16, y: u16, w: u16, h: u16, enc: i32) -> Result<(), RfbError> {
        match enc {
            ENC_DESKTOP_SIZE => {
                if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
                    return Err(RfbError::Protocol("bad desktop size".into()));
                }
                self.fb = Framebuffer::new(w, h);
                Ok(())
            }
            ENC_RAW => {
                if !self.fb.in_bounds(x, y, w, h) {
                    return Err(RfbError::Protocol("raw rect out of bounds".into()));
                }
                let data = r.read_exact(w as usize * h as usize * 4).await?;
                for row in 0..h as usize {
                    for col in 0..w as usize {
                        let i = (row * w as usize + col) * 4;
                        self.fb.put_bgr(x as usize + col, y as usize + row, data[i], data[i + 1], data[i + 2]);
                    }
                }
                Ok(())
            }
            ENC_COPYRECT => {
                let sx = r.u16().await?;
                let sy = r.u16().await?;
                if !self.fb.in_bounds(x, y, w, h) || !self.fb.in_bounds(sx, sy, w, h) {
                    return Err(RfbError::Protocol("copyrect out of bounds".into()));
                }
                self.fb.copy_rect(sx, sy, x, y, w, h);
                Ok(())
            }
            ENC_ZRLE => {
                if !self.fb.in_bounds(x, y, w, h) {
                    return Err(RfbError::Protocol("zrle rect out of bounds".into()));
                }
                let len = r.u32().await? as usize;
                if len > 64 * 1024 * 1024 {
                    return Err(RfbError::Protocol("zrle data too long".into()));
                }
                let compressed = r.read_exact(len).await?;
                let data = self.inflate(&compressed, w as usize * h as usize * 4 + 64 * 1024)?;
                decode_zrle(&mut self.fb, &data, x, y, w, h)
            }
            other => Err(RfbError::Protocol(format!("unsupported encoding {other}"))),
        }
    }

    fn inflate(&mut self, input: &[u8], limit: usize) -> Result<Vec<u8>, RfbError> {
        let mut out = Vec::with_capacity(input.len() * 4);
        let mut consumed = 0usize;
        loop {
            if out.len() >= limit {
                return Err(RfbError::Protocol("zrle data too large".into()));
            }
            out.reserve(64 * 1024);
            let before_in = self.zlib.total_in();
            let before_out = self.zlib.total_out();
            self.zlib
                .decompress_vec(&input[consumed..], &mut out, FlushDecompress::Sync)
                .map_err(|e| RfbError::Protocol(format!("zlib: {e}")))?;
            consumed += (self.zlib.total_in() - before_in) as usize;
            let produced = self.zlib.total_out() - before_out;
            if consumed >= input.len() && out.len() < out.capacity() {
                break;
            }
            if produced == 0 && consumed >= input.len() {
                break;
            }
        }
        Ok(out)
    }
}

/// Decode a ZRLE rectangle from inflated bytes (RFC 6143 §7.7.6), CPIXEL = 3 bytes (B, G, R).
pub fn decode_zrle(fb: &mut Framebuffer, data: &[u8], x: u16, y: u16, w: u16, h: u16) -> Result<(), RfbError> {
    let mut p = 0usize;
    let err = || RfbError::Protocol("zrle truncated".into());
    let take = |n: usize, p: &mut usize| -> Result<std::ops::Range<usize>, RfbError> {
        let r = *p..*p + n;
        if r.end > data.len() {
            return Err(err());
        }
        *p += n;
        Ok(r)
    };
    let mut ty = 0u16;
    while ty < h {
        let th = (h - ty).min(64);
        let mut tx = 0u16;
        while tx < w {
            let tw = (w - tx).min(64);
            let ox = (x + tx) as usize;
            let oy = (y + ty) as usize;
            let sub = data[take(1, &mut p)?.start];
            let n_px = tw as usize * th as usize;
            match sub {
                0 => {
                    let r = take(n_px * 3, &mut p)?;
                    let px = &data[r];
                    for i in 0..n_px {
                        let (cx, cy) = (i % tw as usize, i / tw as usize);
                        fb.put_bgr(ox + cx, oy + cy, px[i * 3], px[i * 3 + 1], px[i * 3 + 2]);
                    }
                }
                1 => {
                    let r = take(3, &mut p)?;
                    let c = &data[r];
                    for cy in 0..th as usize {
                        for cx in 0..tw as usize {
                            fb.put_bgr(ox + cx, oy + cy, c[0], c[1], c[2]);
                        }
                    }
                }
                2..=16 => {
                    let size = sub as usize;
                    let r = take(size * 3, &mut p)?;
                    let pal: Vec<[u8; 3]> = data[r].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
                    let bits = match size {
                        2 => 1,
                        3..=4 => 2,
                        _ => 4,
                    };
                    let row_bytes = (tw as usize * bits).div_ceil(8);
                    let r = take(row_bytes * th as usize, &mut p)?;
                    let packed = &data[r];
                    for cy in 0..th as usize {
                        for cx in 0..tw as usize {
                            let bit = cx * bits;
                            let byte = packed[cy * row_bytes + bit / 8];
                            let shift = 8 - bits - (bit % 8);
                            let idx = ((byte >> shift) as usize) & ((1 << bits) - 1);
                            let c = pal.get(idx).ok_or_else(err)?;
                            fb.put_bgr(ox + cx, oy + cy, c[0], c[1], c[2]);
                        }
                    }
                }
                128 | 130..=255 => {
                    let pal: Vec<[u8; 3]> = if sub == 128 {
                        Vec::new()
                    } else {
                        let r = take((sub as usize - 128) * 3, &mut p)?;
                        data[r].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
                    };
                    let mut i = 0usize;
                    while i < n_px {
                        let (c, run) = if sub == 128 {
                            let r = take(3, &mut p)?;
                            let c = [data[r.start], data[r.start + 1], data[r.start + 2]];
                            (c, read_run(data, &mut p)?)
                        } else {
                            let b = data[take(1, &mut p)?.start];
                            let idx = (b & 0x7f) as usize;
                            let c = *pal.get(idx).ok_or_else(err)?;
                            let run = if b & 0x80 != 0 { read_run(data, &mut p)? } else { 1 };
                            (c, run)
                        };
                        for _ in 0..run {
                            if i >= n_px {
                                return Err(RfbError::Protocol("zrle run too long".into()));
                            }
                            fb.put_bgr(ox + i % tw as usize, oy + i / tw as usize, c[0], c[1], c[2]);
                            i += 1;
                        }
                    }
                }
                _ => return Err(RfbError::Protocol(format!("zrle subencoding {sub}"))),
            }
            tx += 64;
        }
        ty += 64;
    }
    Ok(())
}

fn read_run(data: &[u8], p: &mut usize) -> Result<usize, RfbError> {
    let mut run = 1usize;
    loop {
        let b = *data.get(*p).ok_or_else(|| RfbError::Protocol("zrle truncated".into()))?;
        *p += 1;
        run += b as usize;
        if b != 255 {
            return Ok(run);
        }
        if run > 1 << 24 {
            return Err(RfbError::Protocol("zrle run too long".into()));
        }
    }
}

/// A live Bot Screen session over `/api/display/ws`.
pub struct RfbSession {
    pub input: mpsc::Sender<Vec<u8>>,
}

impl RfbSession {
    /// Connect, handshake and run the decoder. Events go to `events`. Input bytes (key and pointer
    /// messages) go through the returned session's `input` channel. The gateway drops input from a
    /// viewer that does not hold the lease, so the client may always send.
    pub async fn connect(url: &str, events: mpsc::Sender<RfbEvent>) -> Result<RfbSession, RfbError> {
        let req = url
            .into_client_request()
            .map_err(|e| RfbError::Protocol(crate::redact::redact_url(&e.to_string())))?;
        let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(64 << 20))
            .max_frame_size(Some(64 << 20));
        let (ws, _) = tokio::time::timeout(
            Duration::from_secs(15),
            tokio_tungstenite::connect_async_with_config(req, Some(config), true),
        )
        .await
        .map_err(|_| RfbError::Protocol("connect timed out".into()))?
        .map_err(|e| RfbError::Protocol(crate::redact::redact_url(&e.to_string()).chars().take(200).collect()))?;
        let (mut sink, mut stream) = ws.split();
        let (in_tx, in_rx) = mpsc::channel::<Vec<u8>>(64);
        let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(256);
        let close_info = Arc::new(std::sync::Mutex::new((None::<u16>, String::new())));

        tokio::spawn(async move {
            while let Some(b) = out_rx.recv().await {
                if sink.send(WsMessage::Binary(b.into())).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });
        let ci = close_info.clone();
        tokio::spawn(async move {
            while let Some(m) = stream.next().await {
                match m {
                    Ok(WsMessage::Binary(b)) => {
                        if in_tx.send(b.to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Ok(WsMessage::Close(f)) => {
                        if let Some(f) = f
                            && let Ok(mut g) = ci.lock()
                        {
                            *g = (Some(u16::from(f.code)), f.reason.to_string());
                        }
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let out = out_tx.clone();
        let ev = events.clone();
        tokio::spawn(async move {
            let result = run_decoder(ChunkReader::new(in_rx), out, ev.clone()).await;
            let (code, mut reason) = close_info.lock().map(|g| g.clone()).unwrap_or_default();
            if reason.is_empty()
                && let Err(e) = result
            {
                reason = e.to_string();
            }
            let _ = ev.send(RfbEvent::Closed { code, reason }).await;
        });
        Ok(RfbSession { input: out_tx })
    }
}

/// Handshake, then decode updates forever, asking for the next update after each one.
pub async fn run_decoder(mut r: ChunkReader, out: mpsc::Sender<Vec<u8>>, events: mpsc::Sender<RfbEvent>) -> Result<(), RfbError> {
    let (name, w, h) = handshake(&mut r, &out).await?;
    let _ = events.send(RfbEvent::Ready { name, width: w, height: h }).await;
    let send = |m: Vec<u8>| {
        let out = out.clone();
        async move { out.send(m).await.map_err(|_| RfbError::Eof) }
    };
    send(set_pixel_format()).await?;
    send(set_encodings(&[ENC_ZRLE, ENC_COPYRECT, ENC_RAW, ENC_DESKTOP_SIZE])).await?;
    let mut dec = Decoder::new(w, h);
    send(fb_update_request(false, 0, 0, w, h)).await?;
    loop {
        if dec.read_message(&mut r).await? {
            // try_send: if the UI is behind, skip this frame; the next one carries all changes.
            let _ = events.try_send(RfbEvent::Frame(Arc::new(dec.fb.clone())));
            let (w, h) = (dec.fb.width, dec.fb.height);
            send(fb_update_request(true, 0, 0, w, h)).await?;
        }
    }
}

/// Map a character or a named key to an X11 keysym.
pub fn keysym_for_char(c: char) -> u32 {
    let cp = c as u32;
    if (0x20..=0x7e).contains(&cp) || (0xa0..=0xff).contains(&cp) {
        cp
    } else {
        0x0100_0000 | cp
    }
}

pub mod keysym {
    pub const BACKSPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const RETURN: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const END: u32 = 0xff57;
    pub const DELETE: u32 = 0xffff;
    pub const SHIFT_L: u32 = 0xffe1;
    pub const CONTROL_L: u32 = 0xffe3;
    pub const ALT_L: u32 = 0xffe9;
    pub const SUPER_L: u32 = 0xffeb;
    pub const F1: u32 = 0xffbe;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compress, Compression, FlushCompress};

    fn server_init(w: u16, h: u16) -> Vec<u8> {
        let mut v = b"RFB 003.008\n".to_vec();
        v.extend_from_slice(&[1, 1]); // one security type: None
        v.extend_from_slice(&0u32.to_be_bytes());
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0]);
        v.extend_from_slice(&4u32.to_be_bytes());
        v.extend_from_slice(b"test");
        v
    }

    #[tokio::test]
    async fn handshake_and_raw_update() {
        let (tx, rx) = mpsc::channel(16);
        let (out_tx, mut out_rx) = mpsc::channel(16);
        let mut r = ChunkReader::new(rx);
        let mut bytes = server_init(4, 2);
        // update: 1 raw rect at (1,0) 2x1, colours red and blue (B,G,R,X)
        bytes.extend_from_slice(&[0, 0, 0, 1]);
        for v in [1u16, 0, 2, 1] {
            bytes.extend_from_slice(&v.to_be_bytes());
        }
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
        // Split into odd chunks to test reassembly.
        for c in bytes.chunks(5) {
            tx.send(c.to_vec()).await.unwrap();
        }
        let (name, w, h) = handshake(&mut r, &out_tx).await.unwrap();
        assert_eq!((name.as_str(), w, h), ("test", 4, 2));
        assert_eq!(out_rx.recv().await.unwrap(), b"RFB 003.008\n");
        assert_eq!(out_rx.recv().await.unwrap(), vec![1]);
        assert_eq!(out_rx.recv().await.unwrap(), vec![1]);
        let mut d = Decoder::new(w, h);
        assert!(d.read_message(&mut r).await.unwrap());
        assert_eq!(&d.fb.rgba[4..8], &[255, 0, 0, 255]);
        assert_eq!(&d.fb.rgba[8..12], &[0, 0, 255, 255]);
    }

    #[tokio::test]
    async fn rejects_bad_data() {
        let (tx, rx) = mpsc::channel(4);
        let (out_tx, _o) = mpsc::channel(4);
        let mut r = ChunkReader::new(rx);
        tx.send(b"HTTP/1.1 200".to_vec()).await.unwrap();
        assert!(handshake(&mut r, &out_tx).await.is_err());
        // Out-of-bounds raw rect.
        let (tx, rx) = mpsc::channel(4);
        let mut r = ChunkReader::new(rx);
        let mut m = vec![0u8, 0, 0, 1];
        for v in [3u16, 0, 5, 1] {
            m.extend_from_slice(&v.to_be_bytes());
        }
        m.extend_from_slice(&0i32.to_be_bytes());
        tx.send(m).await.unwrap();
        let mut d = Decoder::new(4, 2);
        assert!(d.read_message(&mut r).await.is_err());
        drop(tx);
        assert_eq!(d.read_message(&mut r).await, Err(RfbError::Eof));
    }

    #[test]
    fn zrle_tiles() {
        // 3x1 rect: tile = palette RLE with 2 colours.
        let mut fb = Framebuffer::new(3, 1);
        let data = [130u8, 1, 2, 3, 9, 9, 9, 0x80, 1, 1];
        decode_zrle(&mut fb, &data, 0, 0, 3, 1).unwrap();
        assert_eq!(&fb.rgba[0..4], &[3, 2, 1, 255]);
        assert_eq!(&fb.rgba[4..8], &[3, 2, 1, 255]);
        assert_eq!(&fb.rgba[8..12], &[9, 9, 9, 255]);
        // packed palette, 2 colours, 1 bit/px: row bits 0b101_00000
        let mut fb = Framebuffer::new(3, 1);
        let data = [2u8, 0, 0, 0, 255, 255, 255, 0b1010_0000];
        decode_zrle(&mut fb, &data, 0, 0, 3, 1).unwrap();
        assert_eq!(&fb.rgba[0..3], &[255, 255, 255]);
        assert_eq!(&fb.rgba[4..7], &[0, 0, 0]);
        // truncated data is an error, not a panic
        assert!(decode_zrle(&mut fb, &[0u8, 1], 0, 0, 3, 1).is_err());
        assert!(decode_zrle(&mut fb, &[128u8, 1, 2, 3, 255, 255], 0, 0, 3, 1).is_err());
    }

    #[tokio::test]
    async fn zrle_through_zlib() {
        let mut c = Compress::new(Compression::fast(), true);
        let tile = [1u8, 10, 20, 30]; // solid tile, B=10 G=20 R=30
        let mut z = Vec::with_capacity(256);
        c.compress_vec(&tile, &mut z, FlushCompress::Sync).unwrap();
        let mut m = vec![0u8, 0, 0, 1];
        for v in [0u16, 0, 2, 2] {
            m.extend_from_slice(&v.to_be_bytes());
        }
        m.extend_from_slice(&ENC_ZRLE.to_be_bytes());
        m.extend_from_slice(&(z.len() as u32).to_be_bytes());
        m.extend_from_slice(&z);
        let (tx, rx) = mpsc::channel(4);
        tx.send(m).await.unwrap();
        let mut r = ChunkReader::new(rx);
        let mut d = Decoder::new(2, 2);
        d.read_message(&mut r).await.unwrap();
        assert!(d.fb.rgba.chunks_exact(4).all(|p| p == [30, 20, 10, 255]));
    }

    #[test]
    fn encoders() {
        assert_eq!(key_event(true, 0x61), vec![4, 1, 0, 0, 0, 0, 0, 0x61]);
        assert_eq!(pointer_event(1, 2, 3), vec![5, 1, 0, 2, 0, 3]);
        assert_eq!(set_pixel_format().len(), 20);
        assert_eq!(fb_update_request(true, 0, 0, 1, 1).len(), 10);
        assert_eq!(keysym_for_char('a'), 0x61);
        assert_eq!(keysym_for_char('€'), 0x0100_20ac);
    }
}
