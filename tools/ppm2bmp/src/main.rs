// tools/ppm2bmp/src/main.rs - Zero-dependency PPM to BMP image converter

use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: ppm2bmp <input.ppm> <output.bmp>");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    let file = File::open(input_path)?;
    let mut reader = BufReader::new(file);

    // Parse header tokens
    let mut tokens = Vec::new();
    let mut cur_token = String::new();
    let mut in_comment = false;

    let mut byte = [0u8; 1];
    while tokens.len() < 4 {
        if reader.read_exact(&mut byte).is_err() {
            break;
        }
        let ch = byte[0] as char;

        if in_comment {
            if ch == '\n' || ch == '\r' {
                in_comment = false;
            }
            continue;
        }

        if ch == '#' {
            in_comment = true;
            continue;
        }

        if ch.is_whitespace() {
            if !cur_token.is_empty() {
                tokens.push(cur_token.clone());
                cur_token.clear();
            }
        } else {
            cur_token.push(ch);
        }
    }

    if tokens.len() < 4 || tokens[0] != "P6" {
        return Err("Invalid PPM format (expected P6 header)".into());
    }

    let width: usize = tokens[1].parse()?;
    let height: usize = tokens[2].parse()?;
    let max_val: usize = tokens[3].parse()?;

    if max_val != 255 {
        return Err("Unsupported PPM color depth (expected maxval 255)".into());
    }

    println!("[PPM2BMP] Reading P6 PPM: {}x{}, maxval={}", width, height, max_val);

    // Read RGB pixel data
    let total_pixels = width * height;
    let mut rgb_data = vec![0u8; total_pixels * 3];
    reader.read_exact(&mut rgb_data)?;

    // Prepare BMP output
    let row_stride = ((width * 3 + 3) / 4) * 4;
    let image_size = row_stride * height;
    let file_size = 54 + image_size;

    let out_file = File::create(output_path)?;
    let mut writer = BufWriter::new(out_file);

    // BITMAPFILEHEADER (14 bytes)
    writer.write_all(b"BM")?;
    writer.write_all(&(file_size as u32).to_le_bytes())?;
    writer.write_all(&0u32.to_le_bytes())?; // Reserved
    writer.write_all(&54u32.to_le_bytes())?; // Pixel data offset

    // BITMAPINFOHEADER (40 bytes)
    writer.write_all(&40u32.to_le_bytes())?; // biSize
    writer.write_all(&(width as i32).to_le_bytes())?; // biWidth
    writer.write_all(&(-(height as i32)).to_le_bytes())?; // biHeight (negative for top-down)
    writer.write_all(&1u16.to_le_bytes())?; // biPlanes
    writer.write_all(&24u16.to_le_bytes())?; // biBitCount (24bpp)
    writer.write_all(&0u32.to_le_bytes())?; // biCompression (BI_RGB)
    writer.write_all(&(image_size as u32).to_le_bytes())?; // biSizeImage
    writer.write_all(&2835u32.to_le_bytes())?; // biXPelsPerMeter (72 DPI)
    writer.write_all(&2835u32.to_le_bytes())?; // biYPelsPerMeter (72 DPI)
    writer.write_all(&0u32.to_le_bytes())?; // biClrUsed
    writer.write_all(&0u32.to_le_bytes())?; // biClrImportant

    // Write BGR pixels row by row
    let pad_len = row_stride - (width * 3);
    let pad = [0u8; 4];

    for y in 0..height {
        let row_start = y * width * 3;
        for x in 0..width {
            let px = row_start + x * 3;
            let r = rgb_data[px];
            let g = rgb_data[px + 1];
            let b = rgb_data[px + 2];
            writer.write_all(&[b, g, r])?;
        }
        if pad_len > 0 {
            writer.write_all(&pad[..pad_len])?;
        }
    }

    writer.flush()?;
    println!("[PPM2BMP] Successfully wrote {} bytes to BMP: {}", file_size, output_path);
    Ok(())
}
