use std::f32;

pub fn generate_anchors() -> Vec<[f32; 4]> {
    let mut anchors = Vec::with_capacity(4420);
    let min_boxes = [
        vec![10.0, 16.0, 24.0],
        vec![32.0, 48.0],
        vec![64.0, 96.0],
        vec![128.0, 192.0, 256.0],
    ];
    let feature_map_sizes = [[40, 30], [20, 15], [10, 8], [5, 4]];
    let steps = [8.0, 16.0, 32.0, 64.0];

    for k in 0..feature_map_sizes.len() {
        let min_sizes = &min_boxes[k];
        let step = steps[k];

        for i in 0..feature_map_sizes[k][1] {
            for j in 0..feature_map_sizes[k][0] {
                for min_size in min_sizes {
                    let s_kx = min_size / 320.0;
                    let s_ky = min_size / 240.0;
                    let cx = (j as f32 + 0.5) * step / 320.0;
                    let cy = (i as f32 + 0.5) * step / 240.0;
                    anchors.push([cx, cy, s_kx, s_ky]);
                }
            }
        }
    }
    anchors
}

pub fn decode(loc: &[f32], anchor: &[f32; 4]) -> [f32; 4] {
    let cx = loc[0] * 0.1 * anchor[2] + anchor[0];
    let cy = loc[1] * 0.1 * anchor[3] + anchor[1];
    let w = (loc[2] * 0.2).exp() * anchor[2];
    let h = (loc[3] * 0.2).exp() * anchor[3];
    let x_min = cx - w / 2.0;
    let y_min = cy - h / 2.0;
    let x_max = cx + w / 2.0;
    let y_max = cy + h / 2.0;
    [x_min, y_min, x_max, y_max]
}

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

pub fn nms(boxes: &mut [([f32; 4], f32)], iou_threshold: f32) -> Vec<usize> {
    boxes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut keep = Vec::new();
    let mut suppressed = vec![false; boxes.len()];

    for i in 0..boxes.len() {
        if suppressed[i] {
            continue;
        }
        keep.push(i);
        for j in (i + 1)..boxes.len() {
            if suppressed[j] {
                continue;
            }
            if iou(&boxes[i].0, &boxes[j].0) > iou_threshold {
                suppressed[j] = true;
            }
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_anchors_count() {
        let anchors = generate_anchors();
        assert_eq!(anchors.len(), 4420);
    }

    #[test]
    fn test_generate_anchors_valid_range() {
        let anchors = generate_anchors();
        for anchor in &anchors {
            assert!(anchor[0] >= 0.0 && anchor[0] <= 1.0);
            assert!(anchor[1] >= 0.0 && anchor[1] <= 1.0);
            assert!(anchor[2] > 0.0);
            assert!(anchor[3] > 0.0);
        }
    }

    #[test]
    fn test_decode_box() {
        let loc = [0.5, 0.5, 0.0, 0.0];
        let anchor = [0.5, 0.5, 0.1, 0.1];
        let box_coords = decode(&loc, &anchor);
        assert!(box_coords[0] < box_coords[2]);
        assert!(box_coords[1] < box_coords[3]);
    }

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
    fn test_nms_suppresses_overlapping() {
        let mut boxes = vec![
            ([0.0, 0.0, 1.0, 1.0], 0.9),
            ([0.05, 0.05, 1.05, 1.05], 0.8),
            ([2.0, 2.0, 3.0, 3.0], 0.7),
        ];
        let keep = nms(&mut boxes, 0.5);
        assert_eq!(keep.len(), 2);
    }

    #[test]
    fn test_nms_preserves_disjoint() {
        let mut boxes = vec![
            ([0.0, 0.0, 0.5, 0.5], 0.9),
            ([0.6, 0.0, 1.0, 0.5], 0.8),
            ([0.0, 0.6, 0.5, 1.0], 0.7),
        ];
        let keep = nms(&mut boxes, 0.5);
        assert_eq!(keep.len(), 3);
    }

    #[test]
    fn test_nms_empty() {
        let mut boxes: Vec<([f32; 4], f32)> = vec![];
        let keep = nms(&mut boxes, 0.5);
        assert!(keep.is_empty());
    }

    #[test]
    fn test_nms_single_box() {
        let mut boxes = vec![([0.0, 0.0, 1.0, 1.0], 0.9)];
        let keep = nms(&mut boxes, 0.5);
        assert_eq!(keep.len(), 1);
    }
}
