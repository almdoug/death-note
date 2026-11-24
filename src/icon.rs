//! Ícone do app desenhado em código (caderno escuro, lombada violeta e
//! marcador vermelho). Usado na janela e para gerar `assets/icon.ico`
//! (`cargo run --example make_icon`).

/// Pixels RGBA (não pré-multiplicados) de um ícone `size`×`size`.
pub fn rgba(size: u32) -> Vec<u8> {
    const SS: u32 = 4; // supersampling para bordas suaves
    let n = size as usize;
    let mut out = vec![0u8; n * n * 4];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    // coordenadas normalizadas 0..1
                    let u = (x as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    if let Some(c) = sample(u, v) {
                        let a = c[3] as f32 / 255.0;
                        acc[0] += c[0] as f32 * a;
                        acc[1] += c[1] as f32 * a;
                        acc[2] += c[2] as f32 * a;
                        acc[3] += a;
                    }
                }
            }
            let i = (y as usize * n + x as usize) * 4;
            let total = (SS * SS) as f32;
            if acc[3] > 0.0 {
                out[i] = (acc[0] / acc[3]).round() as u8;
                out[i + 1] = (acc[1] / acc[3]).round() as u8;
                out[i + 2] = (acc[2] / acc[3]).round() as u8;
                out[i + 3] = (acc[3] / total * 255.0).round() as u8;
            }
        }
    }
    out
}

fn rounded_rect(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if u < x0 || u > x1 || v < y0 || v > y1 {
        return false;
    }
    let cx = u.clamp(x0 + r, x1 - r);
    let cy = v.clamp(y0 + r, y1 - r);
    (u - cx).powi(2) + (v - cy).powi(2) <= r * r
}

fn sample(u: f32, v: f32) -> Option<[u8; 4]> {
    // Corpo do caderno
    if !rounded_rect(u, v, 0.10, 0.05, 0.90, 0.95, 0.12) {
        return None;
    }
    // Marcador (fita vermelha com ponta em V)
    let (bx0, bx1) = (0.64, 0.77);
    if u >= bx0 && u <= bx1 {
        let mid = (bx0 + bx1) / 2.0;
        let half = (bx1 - bx0) / 2.0;
        // borda inferior em V: o centro sobe 0.07
        let bottom = 0.40 - 0.07 * (1.0 - (u - mid).abs() / half);
        if v <= bottom {
            return Some([239, 68, 68, 255]);
        }
    }
    // Lombada
    if u < 0.27 {
        // costura
        if (u - 0.235).abs() < 0.012 && ((v * 14.0).fract() < 0.55) && v > 0.10 && v < 0.90 {
            return Some([175, 155, 255, 255]);
        }
        return Some([124, 92, 255, 255]);
    }
    // Linhas da página
    for k in 0..5 {
        let ly = 0.50 + k as f32 * 0.085;
        let len_end = if k == 4 { 0.62 } else { 0.80 };
        if (v - ly).abs() < 0.016 && u > 0.36 && u < len_end {
            return Some([150, 152, 168, 255]);
        }
    }
    Some([30, 31, 36, 255])
}

/// Monta um arquivo .ico com entradas BMP de 32 bits.
#[allow(dead_code)]
pub fn ico(sizes: &[u32]) -> Vec<u8> {
    let mut images: Vec<Vec<u8>> = Vec::new();
    for &s in sizes {
        let px = rgba(s);
        let n = s as usize;
        let mut bmp = Vec::new();
        // BITMAPINFOHEADER (altura dobrada: XOR + AND)
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(s as i32).to_le_bytes());
        bmp.extend_from_slice(&((s * 2) as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&((n * n * 4) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0u8; 16]);
        for y in (0..n).rev() {
            for x in 0..n {
                let i = (y * n + x) * 4;
                bmp.extend_from_slice(&[px[i + 2], px[i + 1], px[i], px[i + 3]]);
            }
        }
        let row = n.div_ceil(32) * 4;
        bmp.extend(std::iter::repeat_n(0u8, row * n));
        images.push(bmp);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (s, img) in sizes.iter().zip(&images) {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[b, b, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += img.len();
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}
