use serde::{Deserialize, Serialize};

/// Axis-aligned rectangle in logical screen pixels (origin top-left).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// Usable area of a monitor: the physical size is divided by the output
    /// scale (Hyprland reports logical positions but physical sizes) and the
    /// reserved margins (bars, docks) are subtracted on every edge.
    /// `reserved` is `[left, top, right, bottom]`.
    pub fn usable_area(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        scale: f64,
        reserved: [i32; 4],
    ) -> Self {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let [left, top, right, bottom] = reserved;
        let logical_w = (f64::from(width) / scale).round() as i32;
        let logical_h = (f64::from(height) / scale).round() as i32;
        Self {
            x: x + left,
            y: y + top,
            width: (logical_w - left - right).max(0),
            height: (logical_h - top - bottom).max(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_area_divides_by_scale_and_subtracts_reserved_edges() {
        let area = Rect::usable_area(0, 0, 3840, 2160, 2.0, [0, 40, 0, 0]);
        assert_eq!(area, Rect::new(0, 40, 1920, 1040));
    }

    #[test]
    fn usable_area_offsets_by_monitor_position() {
        let area = Rect::usable_area(1920, 0, 2560, 1440, 1.0, [10, 20, 30, 40]);
        assert_eq!(area, Rect::new(1930, 20, 2520, 1380));
    }

    #[test]
    fn usable_area_never_goes_negative_and_ignores_bad_scale() {
        let area = Rect::usable_area(0, 0, 100, 100, 0.0, [80, 80, 80, 80]);
        assert_eq!(area, Rect::new(80, 80, 0, 0));
        assert!(area.is_empty());
    }

    #[test]
    fn rect_json_roundtrip_with_defaults() {
        let rect: Rect = serde_json::from_str(r#"{"x": 5, "width": 10}"#).unwrap();
        assert_eq!(rect, Rect::new(5, 0, 10, 0));
        let json = serde_json::to_string(&Rect::new(1, 2, 3, 4)).unwrap();
        assert_eq!(
            serde_json::from_str::<Rect>(&json).unwrap(),
            Rect::new(1, 2, 3, 4)
        );
    }
}
