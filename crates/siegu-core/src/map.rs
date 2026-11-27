use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, NaiveDate};

use crate::database::{haversine_km, MapPoint};

/// How many photo ids a single map marker keeps for its drill-down.
pub const MAX_CLUSTER_PHOTOS: usize = 50;
/// Photos farther apart than this many days start a new trip route.
pub const TRIP_GAP_DAYS: i32 = 3;
/// Even a dense photo run must not become a months-long line. Split it so the
/// timeline shows short, readable outings instead of one endless path.
pub const TRIP_MAX_DAYS: i32 = 14;
/// Consecutive photos farther apart than this start a new route. Bad metadata
/// can put two places thousands of kilometres apart a day apart; never join
/// those with a line. Real flights are intentionally not drawn.
pub const MAX_ROUTE_SEGMENT_KM: f64 = 250.0;
/// Keep the route layer bounded even for very large libraries.
pub const MAX_TRIP_ROUTES: usize = 24;
pub const MAX_ROUTE_POINTS: usize = 200;
/// Ely evaluates every heat point on every paint, so feed it aggregated cells
/// instead of one point per photo.
pub const MAX_HEAT_POINTS: usize = 512;
const HEAT_GRID_DEGREES: f64 = 0.5;

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

/// A chronological path through geotagged photos, drawn as a route layer.
#[derive(Debug, Clone, PartialEq)]
pub struct MapRoute {
    pub id: String,
    pub name: String,
    /// Start date (`YYYY-MM-DD`) used by the map's trip timeline.
    pub date: String,
    pub points: Vec<(f64, f64)>,
}

/// One aggregated density cell for the heat layer.
#[derive(Debug, Clone, PartialEq)]
pub struct MapHeatPoint {
    pub latitude: f64,
    pub longitude: f64,
    pub weight: f32,
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

/// Approximate width of the desktop map viewport, used to turn a longitude
/// span into a zoom level. Assuming a single 256px tile here zoomed far enough
/// in that a globally scattered library opened on empty ocean.
const MAP_FRAME_VIEWPORT_PX: f64 = 900.0;

/// Computes a stable initial view from all geotagged photos. The median keeps
/// the view near the bulk of the library even when a few trips are far away;
/// zoom still fits the full spread so outliers stay reachable.
pub fn frame_map_points(points: &[MapPoint]) -> MapFraming {
    let mut latitudes = Vec::new();
    let mut longitudes = Vec::new();

    for point in points {
        if point.latitude.is_finite() && point.longitude.is_finite() {
            latitudes.push(point.latitude);
            longitudes.push(point.longitude);
        }
    }

    if latitudes.is_empty() {
        return MapFraming::default();
    }
    if latitudes.len() == 1 {
        return MapFraming {
            latitude: latitudes[0],
            longitude: longitudes[0],
            zoom: 4.0,
        };
    }

    let latitude_min = latitudes.iter().copied().fold(f64::MAX, f64::min);
    let latitude_max = latitudes.iter().copied().fold(f64::MIN, f64::max);
    let longitude_min = longitudes.iter().copied().fold(f64::MAX, f64::min);
    let longitude_max = longitudes.iter().copied().fold(f64::MIN, f64::max);

    let span = (longitude_max - longitude_min)
        .max((latitude_max - latitude_min) * 2.0)
        .max(0.02);
    let zoom = (MAP_FRAME_VIEWPORT_PX * 0.75 * 360.0 / (256.0 * span))
        .log2()
        .clamp(2.0, 10.0);
    MapFraming {
        latitude: median(&mut latitudes),
        longitude: median(&mut longitudes),
        zoom,
    }
}

/// Aggregates photos into weighted half-degree cells, keeping only the busiest
/// ones. This bounds Ely's per-paint heat work independently of library size.
pub fn build_heat_points(points: &[MapPoint]) -> Vec<MapHeatPoint> {
    let mut cells: BTreeMap<(i32, i32), (f64, f64, usize)> = BTreeMap::new();
    for point in points {
        if !point.latitude.is_finite() || !point.longitude.is_finite() {
            continue;
        }
        let latitude = point.latitude.clamp(-85.0, 85.0);
        let longitude = point.longitude.clamp(-180.0, 180.0);
        let cell = (
            (latitude / HEAT_GRID_DEGREES).round() as i32,
            (longitude / HEAT_GRID_DEGREES).round() as i32,
        );
        let entry = cells.entry(cell).or_insert((0.0, 0.0, 0));
        entry.0 += latitude;
        entry.1 += longitude;
        entry.2 += 1;
    }
    let mut cells: Vec<_> = cells.into_iter().collect();
    cells.sort_by(|a, b| b.1 .2.cmp(&a.1 .2).then_with(|| a.0.cmp(&b.0)));
    cells
        .into_iter()
        .take(MAX_HEAT_POINTS)
        .map(|(_, (latitude_sum, longitude_sum, count))| MapHeatPoint {
            latitude: latitude_sum / count as f64,
            longitude: longitude_sum / count as f64,
            weight: count as f32,
        })
        .collect()
}

/// Chart hue for a cluster. Busier clusters run hotter; there are eight hues
/// in Ely's chart palette.
pub fn cluster_hue(count: usize) -> usize {
    ((count.max(1) as f64).log2() as usize).min(7)
}

/// Groups photos into short, plausible routes. A date gap, a long run of days,
/// or a jump across the map starts a new route. Only the newest routes are kept
/// and each is thinned to avoid loading a path per photo. The bounds matter:
/// a few bad GPS dates otherwise turn one "trip" into a line across the world.
pub fn build_trip_routes(points: &[MapPoint]) -> Vec<MapRoute> {
    let mut dated: Vec<(i32, &MapPoint)> = points
        .iter()
        .filter_map(|point| {
            if !point.latitude.is_finite() || !point.longitude.is_finite() {
                return None;
            }
            let created = point.created.as_deref()?;
            let date = NaiveDate::parse_from_str(created.get(..10)?, "%Y-%m-%d").ok()?;
            Some((date.num_days_from_ce(), point))
        })
        .collect();
    dated.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let mut groups: Vec<Vec<(i32, &MapPoint)>> = Vec::new();
    for entry in dated {
        let starts_new = match groups.last() {
            Some(group) => {
                let first = group.first().expect("non-empty route group");
                let last = group.last().expect("non-empty route group");
                let last_coord = (
                    last.1.latitude.clamp(-85.0, 85.0),
                    last.1.longitude.clamp(-180.0, 180.0),
                );
                let next_coord = (
                    entry.1.latitude.clamp(-85.0, 85.0),
                    entry.1.longitude.clamp(-180.0, 180.0),
                );
                entry.0 - last.0 > TRIP_GAP_DAYS
                    || entry.0 - first.0 > TRIP_MAX_DAYS
                    || haversine_km(last_coord, next_coord) > MAX_ROUTE_SEGMENT_KM
            }
            None => true,
        };
        if starts_new {
            groups.push(vec![entry]);
        } else {
            groups.last_mut().expect("trip group").push(entry);
        }
    }

    // Keep the newest routes, but return them oldest first so the map's
    // timeline moves forward in time as the user steps through them.
    let mut recent: Vec<_> = groups
        .into_iter()
        .enumerate()
        .filter(|(_, group)| group.len() > 1)
        .collect();
    if recent.len() > MAX_TRIP_ROUTES {
        recent.drain(..recent.len() - MAX_TRIP_ROUTES);
    }
    recent
        .into_iter()
        .filter_map(|(index, group)| {
            let mut seen = HashSet::new();
            let mut coords = Vec::new();
            for (_, point) in &group {
                let key = format!("{:.2}:{:.2}", point.latitude, point.longitude);
                if seen.insert(key) {
                    coords.push((
                        point.latitude.clamp(-85.0, 85.0),
                        point.longitude.clamp(-180.0, 180.0),
                    ));
                    if coords.len() == MAX_ROUTE_POINTS {
                        break;
                    }
                }
            }
            if coords.len() < 2 {
                return None;
            }
            let first_day = group.first().map(|item| item.0).unwrap_or(0);
            let date = NaiveDate::from_num_days_from_ce_opt(first_day)
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let start = group
                .first()
                .map(|item| item.1.location.trim())
                .unwrap_or("");
            let end = group
                .last()
                .map(|item| item.1.location.trim())
                .unwrap_or("");
            let name = if !start.is_empty() && !end.is_empty() && start != end {
                format!("{start} → {end}")
            } else if !start.is_empty() {
                start.to_string()
            } else if !end.is_empty() {
                end.to_string()
            } else if !date.is_empty() {
                date.clone()
            } else {
                format!("Trip {}", index + 1)
            };
            Some(MapRoute {
                id: format!("trip-{first_day}-{index}"),
                name,
                date,
                points: coords,
            })
        })
        .collect()
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
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
    use super::{
        build_heat_points, build_trip_routes, cluster_hue, cluster_map_points, frame_map_points,
        MAX_CLUSTER_PHOTOS, MAX_HEAT_POINTS, TRIP_MAX_DAYS,
    };
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
        assert!(framing.zoom >= 2.0 && framing.zoom <= 10.0);

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

    #[test]
    fn frames_skewed_libraries_near_the_median() {
        let points = vec![
            point("home-1", 52.0, 5.0, ""),
            point("home-2", 52.0, 5.0, ""),
            point("home-3", 52.0, 5.0, ""),
            point("trip", 10.0, -120.0, ""),
        ];
        let framing = frame_map_points(&points);
        assert_eq!((framing.latitude, framing.longitude), (52.0, 5.0));
        assert!(framing.zoom < 6.0);
    }

    #[test]
    fn cluster_hue_scales_with_density() {
        assert_eq!(cluster_hue(0), 0);
        assert_eq!(cluster_hue(1), 0);
        assert_eq!(cluster_hue(8), 3);
        assert_eq!(cluster_hue(10_000), 7);
    }

    #[test]
    fn heat_points_are_aggregated_and_bounded() {
        let mut points = (0..MAX_HEAT_POINTS * 2)
            .map(|index| {
                point(
                    &format!("home-{index}"),
                    1.0 + (index % 3) as f64 * 0.01,
                    2.0,
                    "",
                )
            })
            .collect::<Vec<_>>();
        points.push(point("away", 40.0, 5.0, ""));
        let heat = build_heat_points(&points);
        assert_eq!(heat.len(), 2);
        assert_eq!(heat[0].weight, (MAX_HEAT_POINTS * 2) as f32);
        assert!((heat[0].latitude - 1.01).abs() < 0.01);
        assert_eq!(heat[1].weight, 1.0);

        let scattered = (0..MAX_HEAT_POINTS + 20)
            .map(|index| {
                point(
                    &format!("{index}"),
                    (index % 80) as f64,
                    (index / 80) as f64 * 2.0,
                    "",
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(build_heat_points(&scattered).len(), MAX_HEAT_POINTS);
    }

    #[test]
    fn builds_short_local_trip_routes_from_dated_photos() {
        let dated = |id: &str, lat, lon, location, created: &str| {
            let mut item = point(id, lat, lon, location);
            item.created = Some(created.to_string());
            item
        };
        let points = vec![
            dated("a", 52.0, 5.0, "Amsterdam", "2024-01-01 10:00:00"),
            dated("b", 52.09, 5.12, "Utrecht", "2024-01-02 10:00:00"),
            dated("c", 51.92, 4.48, "Rotterdam", "2024-01-03 10:00:00"),
            dated("d", 40.71, -74.0, "New York", "2024-01-04 10:00:00"),
            dated("e", 1.3, 103.8, "Singapore", "2024-02-01 10:00:00"),
            dated("f", 1.31, 103.82, "Singapore", "2024-02-02 10:00:00"),
        ];
        let routes = build_trip_routes(&points);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].name, "Amsterdam → Rotterdam");
        assert_eq!(routes[0].date, "2024-01-01");
        assert_eq!(routes[0].points.len(), 3);
        assert_eq!(routes[1].name, "Singapore");
        assert_eq!(routes[1].date, "2024-02-01");
        assert_eq!(routes[1].points.len(), 2);
    }

    #[test]
    fn splits_dense_photo_runs_that_span_months() {
        use chrono::{Duration, NaiveDate};

        let base = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
        let points = (0..TRIP_MAX_DAYS + 5)
            .map(|day| {
                let date = base + Duration::days(day as i64);
                let mut item = point(&format!("p{day}"), 52.0 + day as f64 * 0.01, 5.0, "Home");
                item.created = Some(format!("{} 12:00:00", date.format("%Y-%m-%d")));
                item
            })
            .collect::<Vec<_>>();

        let routes = build_trip_routes(&points);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].date, "2024-01-01");
        assert_eq!(routes[1].date, "2024-01-16");
    }
}
