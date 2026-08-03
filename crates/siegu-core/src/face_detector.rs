use std::collections::HashMap;
use std::f32;

fn iou(box1: &[f32; 4], box2: &[f32; 4]) -> f32 {
    let xx1 = f32::max(box1[0], box2[0]);
    let yy1 = f32::max(box1[1], box2[1]);
    let xx2 = f32::min(box1[2], box2[2]);
    let yy2 = f32::min(box1[3], box2[3]);

    let w = f32::max(0.0, xx2 - xx1);
    let h = f32::max(0.0, yy2 - yy1);

    let intersection = w * h;
    let area1 = (box1[2] - box1[0]) * (box1[3] - box1[1]);
    let area2 = (box2[2] - box2[0]) * (box2[3] - box2[1]);

    intersection / (area1 + area2 - intersection)
}

/// Target landmark positions (5 points) used for face alignment, in a
/// 112x112 output canvas. Mirrors the InsightFace alignment reference.
pub const ALIGN_REF: [(f32, f32); 5] = [
    (38.2946, 51.6963),
    (73.5318, 51.5014),
    (56.0252, 71.7366),
    (41.5493, 92.3655),
    (70.7299, 92.2041),
];

/// A face detection produced by the YuNet model: a confidence score, the
/// bounding box `[x1, y1, x2, y2]` and 5 landmarks `[x0,y0, x1,y1, ...]`
/// in the model's 640x640 input space.
#[derive(Debug, Clone)]
pub struct YuNetFace {
    pub score: f32,
    pub bbox: [f32; 4],
    pub landmarks: [f32; 10],
}

/// Decodes YuNet named outputs (cls/obj/bbox/kps at strides 8/16/32) into
/// detections in the 640x640 input space, then applies NMS.
pub fn decode_yunet(
    outputs: &HashMap<String, Vec<f32>>,
    input_size: u32,
    conf_threshold: f32,
    nms_threshold: f32,
) -> Vec<YuNetFace> {
    let mut dets: Vec<YuNetFace> = Vec::new();
    for stride in [8u32, 16, 32] {
        let cols = (input_size / stride) as usize;
        let rows = (input_size / stride) as usize;
        let cls = match outputs.get(&format!("cls_{stride}")) {
            Some(v) => v,
            None => continue,
        };
        let obj = match outputs.get(&format!("obj_{stride}")) {
            Some(v) => v,
            None => continue,
        };
        let bbox = match outputs.get(&format!("bbox_{stride}")) {
            Some(v) => v,
            None => continue,
        };
        let kps = match outputs.get(&format!("kps_{stride}")) {
            Some(v) => v,
            None => continue,
        };
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                let cls_score = cls[idx].clamp(0.0, 1.0);
                let obj_score = obj[idx].clamp(0.0, 1.0);
                let conf = (cls_score * obj_score).sqrt();
                if conf < conf_threshold {
                    continue;
                }
                let cx = (c as f32 + bbox[idx * 4]) * stride as f32;
                let cy = (r as f32 + bbox[idx * 4 + 1]) * stride as f32;
                let bw = bbox[idx * 4 + 2].exp() * stride as f32;
                let bh = bbox[idx * 4 + 3].exp() * stride as f32;
                let mut landmarks = [0f32; 10];
                for n in 0..5 {
                    landmarks[n * 2] = (kps[idx * 10 + n * 2] + c as f32) * stride as f32;
                    landmarks[n * 2 + 1] = (kps[idx * 10 + n * 2 + 1] + r as f32) * stride as f32;
                }
                dets.push(YuNetFace {
                    score: conf,
                    bbox: [cx - bw / 2.0, cy - bh / 2.0, cx + bw / 2.0, cy + bh / 2.0],
                    landmarks,
                });
            }
        }
    }

    dets.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<YuNetFace> = Vec::new();
    for d in dets {
        if kept.iter().any(|k| iou(&k.bbox, &d.bbox) > nms_threshold) {
            continue;
        }
        kept.push(d);
    }
    kept
}

/// Scales YuNet 640x640 detections into the original image coordinate space.
pub fn scale_yunet_faces(faces: &[YuNetFace], sx: f32, sy: f32) -> Vec<YuNetFace> {
    faces
        .iter()
        .map(|f| YuNetFace {
            score: f.score,
            bbox: [
                f.bbox[0] * sx,
                f.bbox[1] * sy,
                f.bbox[2] * sx,
                f.bbox[3] * sy,
            ],
            landmarks: {
                let mut lm = [0f32; 10];
                for (i, v) in f.landmarks.iter().enumerate() {
                    lm[i] = v * if i % 2 == 0 { sx } else { sy };
                }
                lm
            },
        })
        .collect()
}

/// Estimates a partial affine transform (rotation + uniform scale +
/// translation) mapping `src` points onto `dst` points via least squares.
/// Returns a 2x3 matrix `[a, -b, c, b, a, d]` in row-major order, matching
/// OpenCV's `estimateAffinePartial2D`.
pub fn estimate_partial_affine(src: &[(f32, f32)], dst: &[(f32, f32)]) -> [f32; 6] {
    let mut a00 = 0f64;
    let mut a01 = 0f64;
    let mut a02 = 0f64;
    let mut a03 = 0f64;
    let mut a11 = 0f64;
    let mut a12 = 0f64;
    let mut a13 = 0f64;
    let mut a22 = 0f64;
    let mut a23 = 0f64;
    let mut a33 = 0f64;
    let mut b0 = 0f64;
    let mut b1 = 0f64;
    let mut b2 = 0f64;
    let mut b3 = 0f64;
    for (s, t) in src.iter().zip(dst.iter()) {
        let (x, y) = (s.0 as f64, s.1 as f64);
        let (u, v) = (t.0 as f64, t.1 as f64);
        let jac = [[x, -y, 1.0, 0.0], [y, x, 0.0, 1.0]];
        let resid = [u, v];
        for k in 0..4 {
            for l in k..4 {
                let sum: f64 = jac.iter().map(|row| row[k] * row[l]).sum();
                match (k, l) {
                    (0, 0) => a00 += sum,
                    (0, 1) => a01 += sum,
                    (0, 2) => a02 += sum,
                    (0, 3) => a03 += sum,
                    (1, 1) => a11 += sum,
                    (1, 2) => a12 += sum,
                    (1, 3) => a13 += sum,
                    (2, 2) => a22 += sum,
                    (2, 3) => a23 += sum,
                    (3, 3) => a33 += sum,
                    _ => {}
                }
            }
        }
        for k in 0..4 {
            let sum: f64 = jac
                .iter()
                .zip(resid.iter())
                .map(|(row, r)| row[k] * r)
                .sum();
            match k {
                0 => b0 += sum,
                1 => b1 += sum,
                2 => b2 += sum,
                3 => b3 += sum,
                _ => {}
            }
        }
    }
    let mut m = [
        [a00, a01, a02, a03, b0],
        [a01, a11, a12, a13, b1],
        [a02, a12, a22, a23, b2],
        [a03, a13, a23, a33, b3],
    ];
    for i in 0..4 {
        let mut pivot = i;
        for r in (i + 1)..4 {
            if m[r][i].abs() > m[pivot][i].abs() {
                pivot = r;
            }
        }
        m.swap(i, pivot);
        let d = m[i][i];
        if d.abs() < 1e-12 {
            continue;
        }
        for (_c_idx, val) in m[i].iter_mut().enumerate().skip(i) {
            *val /= d;
        }
        for r_idx in 0..4 {
            if r_idx != i {
                let f = m[r_idx][i];
                if f.abs() < 1e-12 {
                    continue;
                }
                let m_i_row_copy: [f64; 5] = m[i]; // Copy the row being read from
                for (c_idx, val) in m[r_idx].iter_mut().enumerate().skip(i) {
                    *val -= f * m_i_row_copy[c_idx];
                }
            }
        }
    }
    let a = m[0][4] as f32;
    let b = m[1][4] as f32;
    let c = m[2][4] as f32;
    let d = m[3][4] as f32;
    [a, -b, c, b, a, d]
}

/// Warps an image with the inverse of a 2x3 affine matrix `m`, bilinear
/// sampling into an output of size `(w, h)`.
pub fn warp_affine(img: &image::RgbImage, m: &[f32; 6], out: (u32, u32)) -> image::RgbImage {
    let (w, h) = (out.0, out.1);
    let mut result = image::RgbImage::new(w, h);
    let (iw, ih) = (img.width(), img.height());
    let a = m[0] as f64;
    let c = m[2] as f64;
    let b = m[3] as f64;
    let d = m[5] as f64;
    let det = a * a + b * b;
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f64, y as f64);
            let (sx, sy) = if det.abs() > 1e-12 {
                let sx = (a * (u - c) + b * (v - d)) / det;
                let sy = (-b * (u - c) + a * (v - d)) / det;
                (sx, sy)
            } else {
                (0.0, 0.0)
            };
            result.put_pixel(x, y, sample_bilinear(img, sx, sy, iw, ih));
        }
    }
    result
}

fn sample_bilinear(img: &image::RgbImage, sx: f64, sy: f64, iw: u32, ih: u32) -> image::Rgb<u8> {
    if sx < 0.0 || sy < 0.0 || sx > (iw - 1) as f64 || sy > (ih - 1) as f64 {
        return image::Rgb([0, 0, 0]);
    }
    let x0 = sx.floor() as i32;
    let y0 = sy.floor() as i32;
    let dx = (sx - x0 as f64) as f32;
    let dy = (sy - y0 as f64) as f32;
    let mut out = [0f32; 3];
    for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let px = (x0 + ox).clamp(0, iw as i32 - 1) as u32;
        let py = (y0 + oy).clamp(0, ih as i32 - 1) as u32;
        let p = img.get_pixel(px, py);
        let wgt = if ox == 0 { 1.0 - dx } else { dx } * if oy == 0 { 1.0 - dy } else { dy };
        for c in 0..3 {
            out[c] += p[c] as f32 * wgt;
        }
    }
    image::Rgb([out[0] as u8, out[1] as u8, out[2] as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iou_identical() {
        let b = [0.0, 0.0, 1.0, 1.0];
        assert!(iou(&b, &b) > 0.99);
    }

    #[test]
    fn test_iou_no_overlap() {
        let b1 = [0.0, 0.0, 1.0, 1.0];
        let b2 = [2.0, 2.0, 3.0, 3.0];
        assert_eq!(iou(&b1, &b2), 0.0);
    }

    #[test]
    fn test_decode_yunet_empty_outputs() {
        let outputs = HashMap::new();
        let dets = decode_yunet(&outputs, 640, 0.5, 0.3);
        assert!(dets.is_empty());
    }

    #[test]
    fn test_decode_yunet_nms_keeps_highest_score() {
        let mut outputs = HashMap::new();
        // stride 8 grid, two high-conf detections in adjacent cells that
        // overlap heavily; NMS should keep only the higher-scoring one.
        let cols = 80usize;
        let mut cls = vec![0f32; cols * 80];
        let mut obj = vec![0f32; cols * 80];
        let mut bbox = vec![0f32; cols * 80 * 4];
        let mut kps = vec![0f32; cols * 80 * 10];
        let a = 10 * cols + 10;
        let b = 11 * cols + 11;
        cls[a] = 0.9;
        obj[a] = 0.9;
        cls[b] = 0.7;
        obj[b] = 0.7;
        // Widen both boxes so they overlap heavily (exp(1.5)*stride ~= 36px
        // on adjacent cells) and force NMS to drop the lower-scoring one.
        for (cell, _) in [(a, 0), (b, 1)] {
            bbox[cell * 4 + 2] = 1.5;
            bbox[cell * 4 + 3] = 1.5;
        }
        outputs.insert("cls_8".into(), cls);
        outputs.insert("obj_8".into(), obj);
        outputs.insert("bbox_8".into(), bbox);
        outputs.insert("kps_8".into(), kps);
        let dets = decode_yunet(&outputs, 640, 0.5, 0.3);
        assert_eq!(dets.len(), 1);
        assert!((dets[0].score - 0.9).abs() < 1e-5);
    }

    #[test]
    fn test_decode_yunet_conf_below_threshold() {
        let mut outputs = HashMap::new();
        let cols = 80usize;
        let mut cls = vec![0f32; cols * 80];
        let mut obj = vec![0f32; cols * 80];
        cls[5] = 0.3;
        obj[5] = 0.3;
        outputs.insert("cls_8".into(), cls);
        outputs.insert("obj_8".into(), obj);
        outputs.insert("bbox_8".into(), vec![0f32; cols * 80 * 4]);
        outputs.insert("kps_8".into(), vec![0f32; cols * 80 * 10]);
        let dets = decode_yunet(&outputs, 640, 0.5, 0.3);
        assert!(dets.is_empty());
    }

    #[test]
    fn test_scale_yunet_faces_scales_box_and_landmarks() {
        let face = YuNetFace {
            score: 0.9,
            bbox: [10.0, 20.0, 100.0, 200.0],
            landmarks: [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        };
        let scaled = scale_yunet_faces(&[face], 2.0, 3.0);
        assert_eq!(scaled[0].bbox, [20.0, 60.0, 200.0, 600.0]);
        assert_eq!(
            scaled[0].landmarks,
            [0.0, 3.0, 4.0, 9.0, 8.0, 15.0, 12.0, 21.0, 16.0, 27.0]
        );
    }

    #[test]
    fn test_estimate_partial_affine_identity_on_aligned_points() {
        // When src == dst, the partial affine is a pure identity transform.
        let pts = [
            (38.2946, 51.6963),
            (73.5318, 51.5014),
            (56.0252, 71.7366),
            (41.5493, 92.3655),
            (70.7299, 92.2041),
        ];
        let m = estimate_partial_affine(&pts, &pts);
        // Identity partial affine: a ~ 1, b ~ 0, c,d ~ 0
        assert!((m[0] - 1.0).abs() < 1e-3);
        assert!(m[1].abs() < 1e-3);
        assert!(m[3].abs() < 1e-3);
        assert!(m[2].abs() < 1e-3);
        assert!(m[5].abs() < 1e-3);
    }

    #[test]
    fn test_warp_affine_output_size() {
        let img = image::RgbImage::from_fn(10, 10, |x, y| image::Rgb([x as u8, y as u8, 0]));
        let m = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let out = warp_affine(&img, &m, (5, 5));
        assert_eq!((out.width(), out.height()), (5, 5));
    }
}
