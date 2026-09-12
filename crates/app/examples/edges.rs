//! Finds where a horizontal scan line changes colour, to measure paddings and
//! alignments without eyeballing a screenshot.
//! edges -- img.png Y

use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).expect("image").to_rgba8();
    let y: u32 = a[2].parse().expect("Y");

    // Errors are ignored on purpose: piping into `head` closes the pipe, and a
    // measuring tool has no business panicking over that.
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut prev = None;
    for x in 0..img.width() {
        let p = img.get_pixel(x, y);
        let cur = (p[0], p[1], p[2]);
        if Some(cur) != prev {
            let _ = writeln!(out, "x={x:>4}  rgb({:>3},{:>3},{:>3})", cur.0, cur.1, cur.2);
            prev = Some(cur);
        }
    }
}
