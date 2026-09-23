//! Stacks two crops to compare two renderings of the same area.
//! compare -- out.png  a.png X Y W H  b.png X Y W H  SCALE
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = &a[1];
    let scale: u32 = a[12].parse().unwrap();
    let mut crops = Vec::new();
    for base in [2usize, 7] {
        let img = image::open(&a[base]).unwrap().to_rgba8();
        let (x, y, w, h): (u32, u32, u32, u32) = (
            a[base + 1].parse().unwrap(),
            a[base + 2].parse().unwrap(),
            a[base + 3].parse().unwrap(),
            a[base + 4].parse().unwrap(),
        );
        let sub = image::imageops::crop_imm(&img, x, y, w, h).to_image();
        crops.push(image::imageops::resize(
            &sub,
            w * scale,
            h * scale,
            image::imageops::FilterType::Lanczos3,
        ));
    }
    let width = crops.iter().map(|c| c.width()).max().unwrap();
    let gap = 8;
    let height: u32 = crops.iter().map(|c| c.height()).sum::<u32>() + gap;
    let mut canvas =
        image::RgbaImage::from_pixel(width, height, image::Rgba([0x33, 0x36, 0x3E, 255]));
    let mut y = 0;
    for c in &crops {
        image::imageops::overlay(&mut canvas, c, 0, y as i64);
        y += c.height() + gap;
    }
    canvas.save(out).unwrap();
    println!("{out} -> {width}x{height}");
}
