use base64::Engine;
use image::{DynamicImage, ImageFormat};
use std::fs::File;
use std::io::BufReader;

const THUMB_SIZE: u32 = 320;

fn is_heic_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".heic") || lower.ends_with(".heif")
}

fn open_image(path: &str) -> Option<DynamicImage> {
    if is_heic_file(path) {
        open_heic(path)
    } else {
        image::open(path).ok()
    }
}

fn open_heic(path: &str) -> Option<DynamicImage> {
    let data = std::fs::read(path).ok()?;
    let output = heic::DecoderConfig::new()
        .decode(&data, heic::PixelLayout::Rgba8)
        .ok()?;
    let img = image::RgbaImage::from_raw(output.width, output.height, output.data)?;
    Some(DynamicImage::ImageRgba8(img))
}

pub fn read_exif_orientation(path: &str) -> u16 {
    let Ok(file) = File::open(path) else {
        return 1;
    };
    let mut buf = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut buf) else {
        return 1;
    };
    let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) else {
        return 1;
    };
    field.value.get_uint(0).unwrap_or(1) as u16
}

pub fn apply_orientation(img: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.fliph().rotate270(),
        6 => img.rotate90(),
        7 => img.flipv().rotate270(),
        8 => img.rotate270(),
        _ => img,
    }
}

pub fn needs_thumbnail(encoded: &str) -> bool {
    encoded.is_empty()
}

pub fn is_video_ext(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".mp4")
        || lower.ends_with(".mkv")
        || lower.ends_with(".mov")
        || lower.ends_with(".avi")
        || lower.ends_with(".webm")
        || lower.ends_with(".flv")
        || lower.ends_with(".wmv")
        || lower.ends_with(".m4v")
        || lower.ends_with(".3gp")
}

pub fn generate_thumbnail(path: &str) -> Option<String> {
    if is_video_ext(path) {
        generate_video_thumbnail(path)
    } else {
        generate_image_thumbnail(path)
    }
}

fn generate_image_thumbnail(path: &str) -> Option<String> {
    let img = open_image(path)?;
    let orientation = read_exif_orientation(path);
    let img = apply_orientation(img, orientation);
    let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
    let mut buf = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut buf, ImageFormat::Jpeg).ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf.get_ref())
    ))
}

fn generate_video_thumbnail(path: &str) -> Option<String> {
    let mut ctx = ffmpeg_next::format::input(&path).ok()?;
    let streams = ctx.streams();
    let best = streams.best(ffmpeg_next::media::Type::Video)?;
    let stream_index = best.index();
    let time_base = best.time_base();
    let params = best.parameters().clone();
    drop(best);
    drop(streams);
    let codec_id = params.id();
    let codec = ffmpeg_next::codec::decoder::find(codec_id)?;
    let codec_ctx = ffmpeg_next::codec::context::Context::from_parameters(params).ok()?;
    let mut decoder = codec_ctx.decoder().open_as(codec).ok()?;
    let seek_ts = ((time_base.denominator() as f64 / time_base.numerator() as f64) * 0.5) as i64;
    let _ = ctx.seek(seek_ts, ..seek_ts);

    for (stream_idx, packet) in ctx.packets() {
        if stream_idx.index() != stream_index {
            continue;
        }

        let _ = decoder.send_packet(&packet);

        loop {
            let mut frame = ffmpeg_next::frame::Video::empty();
            match decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    if let Some(result) = process_video_frame(&frame) {
                        return Some(result);
                    }
                }
                Err(_) => break,
            }
        }
    }

    let _ = decoder.send_packet(&ffmpeg_next::packet::Packet::empty());

    loop {
        let mut frame = ffmpeg_next::frame::Video::empty();
        match decoder.receive_frame(&mut frame) {
            Ok(()) => {
                if let Some(result) = process_video_frame(&frame) {
                    return Some(result);
                }
            }
            Err(_) => break,
        }
    }

    None
}

fn process_video_frame(frame: &ffmpeg_next::frame::Video) -> Option<String> {
    let mut rgb = ffmpeg_next::frame::Video::empty();
    let mut scaler = match ffmpeg_next::software::scaling::context::Context::get(
        frame.format(),
        frame.width(),
        frame.height(),
        ffmpeg_next::format::Pixel::RGB24,
        THUMB_SIZE,
        THUMB_SIZE,
        ffmpeg_next::software::scaling::Flags::BILINEAR,
    ) {
        Ok(s) => s,
        Err(_) => return None,
    };
    scaler.run(frame, &mut rgb).ok()?;
    let pixels = rgb.data(0).to_vec();
    let img = image::RgbImage::from_raw(THUMB_SIZE, THUMB_SIZE, pixels)?;
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, ImageFormat::Jpeg)
        .ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf.get_ref())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_image(path: &str) {
        let img = image::RgbImage::new(100, 100);
        img.save(path).unwrap();
    }

    fn create_corrupt_file(path: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(b"not an image").unwrap();
    }

    #[test]
    fn test_is_video_ext() {
        assert!(is_video_ext("video.mp4"));
        assert!(is_video_ext("video.MOV"));
        assert!(!is_video_ext("photo.jpg"));
        assert!(!is_video_ext("photo.png"));
    }

    #[test]
    fn test_needs_thumbnail_empty() {
        assert!(needs_thumbnail(""));
    }

    #[test]
    fn test_needs_thumbnail_non_empty() {
        assert!(!needs_thumbnail("data:image/jpeg;base64,abc123"));
    }

    #[test]
    fn test_generate_image_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        let path_str = path.to_str().unwrap();
        create_test_image(path_str);
        let result = generate_image_thumbnail(path_str);
        assert!(result.is_some());
        let data_url = result.unwrap();
        assert!(data_url.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn test_generate_image_thumbnail_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.jpg");
        let path_str = path.to_str().unwrap();
        create_corrupt_file(path_str);
        let result = generate_image_thumbnail(path_str);
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_image_thumbnail_nonexistent() {
        let result = generate_image_thumbnail("/nonexistent/path.jpg");
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_video_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.mp4");
        let path_str = path.to_str().unwrap();

        let ffmpeg_output = std::process::Command::new("ffmpeg")
            .args([
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:size=100x100:d=1",
                "-frames:v",
                "1",
                "-y",
                path_str,
            ])
            .output();

        match ffmpeg_output {
            Ok(out) if out.status.success() => {
                let result = generate_video_thumbnail(path_str);
                assert!(result.is_some());
                let data_url = result.unwrap();
                assert!(data_url.starts_with("data:image/jpeg;base64,"));

                let encoded = data_url.strip_prefix("data:image/jpeg;base64,").unwrap();
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .unwrap();
                let img = image::load_from_memory(&bytes).unwrap();
                assert_eq!(img.width(), THUMB_SIZE);
                assert_eq!(img.height(), THUMB_SIZE);
            }
            _ => {
                eprintln!("Skipping video thumbnail test: ffmpeg not available");
            }
        }
    }

    #[test]
    fn test_generate_video_thumbnail_nonexistent() {
        let result = generate_video_thumbnail("/nonexistent/video.mp4");
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_thumbnail_dispatches_to_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.jpg");
        let path_str = path.to_str().unwrap();
        create_test_image(path_str);
        let result = generate_thumbnail(path_str);
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn test_generate_thumbnail_dispatches_to_video() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        let path_str = path.to_str().unwrap();

        let ffmpeg_output = std::process::Command::new("ffmpeg")
            .args([
                "-f",
                "lavfi",
                "-i",
                "color=c=red:size=64x64:d=1",
                "-frames:v",
                "1",
                "-y",
                path_str,
            ])
            .output();

        match ffmpeg_output {
            Ok(out) if out.status.success() => {
                let result = generate_thumbnail(path_str);
                assert!(result.is_some());
                assert!(result.unwrap().starts_with("data:image/jpeg;base64,"));
            }
            _ => {
                eprintln!("Skipping dispatch video test: ffmpeg not available");
            }
        }
    }

    #[test]
    fn test_generate_thumbnail_invalid_path() {
        let result = generate_thumbnail("/nonexistent/file.xyz");
        assert!(result.is_none());
    }
}
