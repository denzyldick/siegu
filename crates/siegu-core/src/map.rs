use std::collections::BTreeMap;

use crate::database::MapPoint;

/// How many photo ids a single map marker keeps for its drill-down.
pub const MAX_CLUSTER_PHOTOS: usize = 50;

/// A group of geotagged photos close enough to share one map marker.
#[derive(Debug, Clone, PartialEq)]
pub struct MapCluster {
    /// Stable key derived from the rounded position.
    pub key: String,
    /// Rounded latitude of the marker.
    pub latitude: f64,
    /// Rounded longitude of the marker.
    pub longitude: f64,
    /// First readable location name in the group, if any.
    pub label: String,
    /// Number of geotagged photos in the group.
    pub count: usize,
    /// Photo ids to open when the marker is pressed, capped for UI performance.
    pub photo_ids: Vec<String>,
}

/// Where a map should open before the user pans or zooms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapFraming {
    /// Center latitude.
    pub latitude: f64,
    /// Center longitude.
    pub longitude: f64,
    /// Initial zoom level.
    pub zoom: f64,
}

impl Default for MapFraming {
    fn default() -> Self {
        Self {
            latitude: 20.0,
            longitude: 0.0,
            zoom: 2.0,
        }
    }
}

/// Groups points by rounded coordinates so a large library does not create one
/// UI marker per photo. The result is sorted by key and every key is unique.
pub fn cluster_map_points(points: &[MapPoint], decimals: u32) -> Vec<MapCluster> {
    let factor = 10_f64.powi(decimals as i32);
    let mut groups: BTreeMap<String, MapCluster> = BTreeMap::new();

    for point in points {
        if !point.latitude.is_finite() || !point.longitude.is_finite() {
            continue;
        }
        let latitude = round_coordinate(point.latitude, factor);
        let longitude = round_coordinate(point.longitude, factor);
        let key = format!(
            "{latitude:.precision$}:{longitude:.precision$}",
            precision = decimals as usize
        );
        let label = point.location.trim();
        let group = groups.entry(key.clone()).or_insert_with(|| MapCluster {
            key,
            latitude,
            longitude,
            label: label.to_string(),
            count: 0,
            photo_ids: Vec::new(),
        });
        if group.label.is_empty() && !label.is_empty() {
            group.label = label.to_string();
        }
        group.count += 1;
        if group.photo_ids.len() < MAX_CLUSTER_PHOTOS {
            group.photo_ids.push(point.id.clone());
        }
    }

    groups.into_values().collect()
}

/// Computes a stable initial view from all geotagged photos. A single point
/// opens at a street-level zoom; an empty library opens on the world view.
pub fn frame_map_points(points: &[MapPoint]) -> MapFraming {
    let mut latitude_min = f64::MAX;
    let mut latitude_max = f64::MIN;
    let mut longitude_min = f64::MAX;
    let mut longitude_max = f64::MIN;
    let mut count = 0usize;

    for point in points {
        if !point.latitude.is_finite() || !point.longitude.is_finite() {
            continue;
        }
        latitude_min = latitude_min.min(point.latitude);
        latitude_max = latitude_max.max(point.latitude);
        longitude_min = longitude_min.min(point.longitude);
        longitude_max = longitude_max.max(point.longitude);
        count += 1;
    }

    if count == 0 {
        return MapFraming::default();
    }
    if count == 1 {
        return MapFraming {
            latitude: latitude_min,
            longitude: longitude_min,
            zoom: 4.0,
        };
    }

    let span = (longitude_max - longitude_min)
        .max((latitude_max - latitude_min) * 2.0)
        .max(0.02);
    let zoom = (256.0 * 0.75 / (span / 360.0)).log2().clamp(0.0, 10.0);
    MapFraming {
        latitude: (latitude_min + latitude_max) / 2.0,
        longitude: (longitude_min + longitude_max) / 2.0,
        zoom,
    }
}

fn round_coordinate(value: f64, factor: f64) -> f64 {
    let rounded = (value * factor).round() / factor;
    if rounded.abs() < 0.5 / factor {
        0.0
    } else {
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::{cluster_map_points, frame_map_points, MAX_CLUSTER_PHOTOS};
    use crate::database::MapPoint;

    fn point(id: &str, latitude: f64, longitude: f64, location: &str) -> MapPoint {
        MapPoint {
            id: id.to_string(),
            latitude,
            longitude,
            location: location.to_string(),
            created: None,
        }
    }

    #[test]
    fn groups_nearby_points_into_one_stable_marker() {
        let clusters = cluster_map_points(
            &[
                point("a", 51.5001, -0.1245, "London"),
                point("b", 51.5004, -0.1248, ""),
                point("c", 40.7128, -74.006, "New York"),
            ],
            2,
        );
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].label, "New York");
        assert_eq!(clusters[0].latitude, 40.71);
        assert_eq!(clusters[1].label, "London");
        assert_eq!(clusters[1].count, 2);
        assert_eq!(clusters[1].photo_ids, vec!["a", "b"]);
        assert_ne!(clusters[0].key, clusters[1].key);
    }

    #[test]
    fn ignores_points_without_finite_coordinates() {
        let clusters = cluster_map_points(
            &[
                point("a", f64::NAN, 1.0, ""),
                point("b", 1.0, f64::INFINITY, ""),
                point("c", 1.0, 2.0, "Valid"),
            ],
            3,
        );
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].photo_ids, vec!["c"]);
    }

    #[test]
    fn caps_photo_ids_but_keeps_the_true_count() {
        let points = (0..MAX_CLUSTER_PHOTOS + 10)
            .map(|index| point(&format!("{index}"), 1.0, 2.0, "Home"))
            .collect::<Vec<_>>();
        let clusters = cluster_map_points(&points, 2);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].count, MAX_CLUSTER_PHOTOS + 10);
        assert_eq!(clusters[0].photo_ids.len(), MAX_CLUSTER_PHOTOS);
    }

    #[test]
    fn frames_points_without_network_data() {
        let framing = frame_map_points(&[point("a", 10.0, -20.0, ""), point("b", 20.0, 40.0, "")]);
        assert_eq!(framing.latitude, 15.0);
        assert_eq!(framing.longitude, 10.0);
        assert!(framing.zoom > 0.0 && framing.zoom <= 10.0);

        let single = frame_map_points(&[point("a", 1.0, 2.0, "")]);
        assert_eq!(
            (single.latitude, single.longitude, single.zoom),
            (1.0, 2.0, 4.0)
        );

        let empty = frame_map_points(&[point("a", f64::NAN, 2.0, "")]);
        assert_eq!(
            (empty.latitude, empty.longitude, empty.zoom),
            (20.0, 0.0, 2.0)
        );
    }
}
