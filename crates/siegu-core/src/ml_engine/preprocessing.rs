use ndarray::Array4;

pub fn clip_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 224, 224, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 224, 224));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.48145466) / 0.26862954;
        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.4578275) / 0.2613026;
        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.40821073) / 0.2757771;
    }
    input
}

pub fn aesthetics_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 384, 384, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 384, 384));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 127.5 - 1.0;
        input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 127.5 - 1.0;
        input[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 127.5 - 1.0;
    }
    input
}

pub fn nsfw_preprocess(img: &image::RgbImage) -> Array4<f32> {
    clip_preprocess(img)
}

pub fn ocr_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 320, 48, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 48, 320));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
    }
    input
}

pub fn yolo_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 640, 640, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 640, 640));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
        input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
        input[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
    }
    input
}

pub fn face_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 320, 240, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 240, 320));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.0) / 128.0;
        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.0) / 128.0;
        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.0) / 128.0;
    }
    input
}

pub fn arcface_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 112, 112, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 112, 112));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.5) / 128.0;
        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.5) / 128.0;
        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.5) / 128.0;
    }
    input
}

pub fn midas_preprocess(img: &image::RgbImage) -> Array4<f32> {
    let resized = image::imageops::resize(img, 256, 256, image::imageops::FilterType::Triangle);
    let mut input = Array4::<f32>::zeros((1, 3, 256, 256));
    for (x, y, pixel) in resized.enumerate_pixels() {
        input[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
        input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
        input[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
    }
    input
}

pub fn blip_preprocess(img: &image::RgbImage) -> Array4<f32> {
    clip_preprocess(img)
}
