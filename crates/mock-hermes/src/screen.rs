//! A fake Bot Screen: a tiny RFB 3.8 server (security type None) that draws a mock browser
//! with a sign-in form. Typed keys go into the focused field while a human holds the lease.

use flate2::{Compress, Compression, FlushCompress};

pub const W: u16 = 1280;
pub const H: u16 = 800;

/// Simple 5x7 bitmap font for ASCII 32..127 (enough to render mock text).
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        '.' => [0, 0, 0, 0, 0, 0x0c, 0x0c],
        ':' => [0, 0x0c, 0x0c, 0, 0x0c, 0x0c, 0],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '-' => [0, 0, 0, 0x1f, 0, 0, 0],
        '@' => [0x0e, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0e],
        '*' => [0, 0x15, 0x0e, 0x1f, 0x0e, 0x15, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0x1f],
        ',' => [0, 0, 0, 0, 0x0c, 0x04, 0x08],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0, 0x04],
        _ => [0; 7],
    }
}

#[derive(Clone)]
pub struct Screen {
    pub px: Vec<[u8; 3]>, // RGB
    pub email: String,
    pub password_len: usize,
    pub focus: u8, // 0 email, 1 password
    pub signed_in: bool,
    pub cursor: (u16, u16),
}

const BG: [u8; 3] = [236, 239, 243];
const CHROME: [u8; 3] = [52, 58, 70];
const CARD: [u8; 3] = [255, 255, 255];
const INK: [u8; 3] = [30, 34, 40];
const MUTED: [u8; 3] = [120, 128, 140];
const ACCENT: [u8; 3] = [46, 110, 230];
const FIELD: [u8; 3] = [246, 247, 249];

impl Screen {
    pub fn new() -> Self {
        let mut s = Self { px: vec![BG; W as usize * H as usize], email: String::new(), password_len: 0, focus: 0, signed_in: false, cursor: (0, 0) };
        s.redraw();
        s
    }

    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..(y + h).min(H as usize) {
            for xx in x..(x + w).min(W as usize) {
                self.px[yy * W as usize + xx] = c;
            }
        }
    }

    fn text(&mut self, x: usize, y: usize, s: &str, scale: usize, c: [u8; 3]) {
        for (i, ch) in s.chars().enumerate() {
            let g = glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        self.rect(x + i * 6 * scale + col * scale, y + row * scale, scale, scale, c);
                    }
                }
            }
        }
    }

    pub fn redraw(&mut self) {
        self.px.fill(BG);
        // Browser chrome.
        self.rect(0, 0, W as usize, 44, CHROME);
        for (i, c) in [[230u8, 90, 90], [230, 190, 80], [100, 200, 120]].iter().enumerate() {
            self.rect(16 + i * 20, 16, 12, 12, *c);
        }
        self.rect(110, 9, 900, 26, [74, 80, 94]);
        self.text(124, 16, "HTTPS://EXAMPLE-JOBS.TEST/LOGIN", 2, [210, 215, 225]);
        // Card.
        let (cx, cy) = (440usize, 150usize);
        self.rect(cx, cy, 400, 430, CARD);
        if self.signed_in {
            self.text(cx + 40, cy + 60, "WELCOME BACK", 4, INK);
            self.text(cx + 40, cy + 120, "YOU ARE SIGNED IN.", 2, MUTED);
            self.rect(cx + 40, cy + 170, 320, 6, [100, 200, 120]);
            return;
        }
        self.text(cx + 40, cy + 40, "SIGN IN", 4, INK);
        self.text(cx + 40, cy + 90, "EXAMPLE JOBS", 2, MUTED);
        self.text(cx + 40, cy + 140, "EMAIL", 2, MUTED);
        let f0 = if self.focus == 0 { ACCENT } else { [210, 214, 220] };
        self.rect(cx + 38, cy + 162, 324, 44, f0);
        self.rect(cx + 40, cy + 164, 320, 40, FIELD);
        let email: String = self.email.chars().take(24).collect();
        self.text(cx + 52, cy + 178, &email, 2, INK);
        self.text(cx + 40, cy + 230, "PASSWORD", 2, MUTED);
        let f1 = if self.focus == 1 { ACCENT } else { [210, 214, 220] };
        self.rect(cx + 38, cy + 252, 324, 44, f1);
        self.rect(cx + 40, cy + 254, 320, 40, FIELD);
        let dots = "*".repeat(self.password_len.min(24));
        self.text(cx + 52, cy + 268, &dots, 2, INK);
        self.rect(cx + 40, cy + 330, 320, 48, ACCENT);
        self.text(cx + 158, cy + 346, "CONTINUE", 2, [255, 255, 255]);
    }

    /// Handle a key from the lease holder.
    pub fn key(&mut self, keysym: u32) {
        if self.signed_in {
            return;
        }
        match keysym {
            0xff09 => self.focus = 1 - self.focus,
            0xff0d => {
                if self.focus == 0 {
                    self.focus = 1
                } else if !self.email.is_empty() && self.password_len > 0 {
                    self.signed_in = true
                }
            }
            0xff08 => {
                if self.focus == 0 {
                    self.email.pop();
                } else {
                    self.password_len = self.password_len.saturating_sub(1);
                }
            }
            0x20..=0x7e => {
                if self.focus == 0 {
                    if self.email.len() < 64 {
                        self.email.push(keysym as u8 as char);
                    }
                } else {
                    // A real screen would type the password; the mock only counts it.
                    self.password_len += 1;
                }
            }
            _ => {}
        }
        self.redraw();
    }

    pub fn click(&mut self, x: u16, y: u16) {
        let (cx, cy) = (440u16, 150u16);
        if (cx + 40..cx + 360).contains(&x) {
            if (cy + 164..cy + 204).contains(&y) {
                self.focus = 0;
            } else if (cy + 254..cy + 294).contains(&y) {
                self.focus = 1;
            } else if (cy + 330..cy + 378).contains(&y) && !self.email.is_empty() && self.password_len > 0 {
                self.signed_in = true;
            }
        }
        self.redraw();
    }

    /// Encode the whole screen as one ZRLE rectangle payload (zlib stream continues across calls).
    pub fn zrle(&self, z: &mut Compress) -> Vec<u8> {
        let mut raw = Vec::with_capacity(W as usize * H as usize * 3 / 8);
        let mut ty = 0usize;
        while ty < H as usize {
            let th = (H as usize - ty).min(64);
            let mut tx = 0usize;
            while tx < W as usize {
                let tw = (W as usize - tx).min(64);
                // Plain RLE tile (subencoding 128): runs of identical pixels.
                raw.push(128);
                let mut run_c: Option<[u8; 3]> = None;
                let mut run = 0usize;
                let flush = |raw: &mut Vec<u8>, c: [u8; 3], run: usize| {
                    raw.extend_from_slice(&[c[2], c[1], c[0]]);
                    let mut left = run - 1;
                    while left >= 255 {
                        raw.push(255);
                        left -= 255;
                    }
                    raw.push(left as u8);
                };
                for yy in ty..ty + th {
                    for xx in tx..tx + tw {
                        let c = self.px[yy * W as usize + xx];
                        if run_c == Some(c) {
                            run += 1;
                        } else {
                            if let Some(rc) = run_c {
                                flush(&mut raw, rc, run);
                            }
                            run_c = Some(c);
                            run = 1;
                        }
                    }
                }
                if let Some(rc) = run_c {
                    flush(&mut raw, rc, run);
                }
                tx += 64;
            }
            ty += 64;
        }
        let mut out = Vec::with_capacity(raw.len() / 4 + 1024);
        let mut input = &raw[..];
        loop {
            out.reserve(256 * 1024);
            let before = z.total_in();
            let _ = z.compress_vec(input, &mut out, FlushCompress::Sync);
            let used = (z.total_in() - before) as usize;
            input = &input[used..];
            if input.is_empty() && out.len() < out.capacity() {
                break;
            }
        }
        out
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

pub fn new_zlib() -> Compress {
    Compress::new(Compression::fast(), true)
}
