//! GPGL command generation for Silhouette cutters (Graphtec protocol).
//!
//! Protocol facts follow the reverse-engineering done by the
//! inkscape-silhouette and robocut projects; this is an independent
//! implementation. Every command is ASCII terminated by ETX (0x03).
//! Coordinates are in device units of 1/20 mm, given as `y,x`
//! (y = feed direction / down the mat, x = across), origin top-left.

use kurbo::Point;
use serde::{Deserialize, Serialize};

pub const ETX: u8 = 0x03;
pub const ESC: u8 = 0x1b;
/// Device units per millimetre.
pub const SU_PER_MM: f64 = 20.0;

pub fn su(mm: f64) -> i64 {
    (mm * SU_PER_MM).round() as i64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    Blade,
    Pen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatKind {
    /// Cutting mat 8″ × 12″ (Portrait).
    Portrait8x12,
    /// Cutting mat 8.5″ × 12″ (Portrait).
    Portrait8_5x12,
    /// Media without a mat.
    None,
}

impl MatKind {
    /// `TG` parameter and usable size (width, length) in inches.
    fn params(self) -> (u8, Option<(f64, f64)>) {
        match self {
            MatKind::Portrait8x12 => (3, Some((8.0, 12.0))),
            MatKind::Portrait8_5x12 => (11, Some((8.5, 12.0))),
            MatKind::None => (0, None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutSettings {
    pub tool: Tool,
    /// Force 1..=33.
    pub pressure: u8,
    /// Speed 1..=30 (Cameo 4 / Portrait 3 family).
    pub speed: u8,
    /// AutoBlade depth 0..=10; `None` leaves the blade as it is.
    pub depth: Option<u8>,
    pub mat: MatKind,
    /// Blade tip offset in mm (0.9 mm standard blade). Compensated by the device.
    #[serde(default = "default_blade_offset")]
    pub blade_offset: f64,
    /// Lift the blade at sharp corners.
    #[serde(default)]
    pub lift_corners: bool,
}

fn default_blade_offset() -> f64 {
    0.9
}

impl Default for CutSettings {
    fn default() -> Self {
        Self { tool: Tool::Blade, pressure: 10, speed: 5, depth: Some(1), mat: MatKind::Portrait8x12, blade_offset: 0.9, lift_corners: false }
    }
}

/// Device limits (width/length of the cuttable area in mm).
#[derive(Debug, Clone, Copy)]
pub struct DeviceLimits {
    pub width_mm: f64,
    pub length_mm: f64,
}

pub const PORTRAIT3: DeviceLimits = DeviceLimits { width_mm: 203.0, length_mm: 18290.0 };

/// A complete job split into phases so the sender can report progress and
/// wait for the device between plot chunks.
#[derive(Debug, Clone, Default)]
pub struct Job {
    /// Media set-up (mat, boundaries) plus registration-mark settings.
    pub setup: Vec<String>,
    /// Registration-mark search command; its reply must be read (≈ 40 s).
    pub registration: Option<String>,
    /// One pass per cut setting (e.g. kiss-cut, then through-cut).
    pub passes: Vec<Pass>,
    pub trailer: Vec<String>,
}

/// Tool settings followed by the move/draw commands cut with them.
#[derive(Debug, Clone, Default)]
pub struct Pass {
    pub tool: Vec<String>,
    pub plot: Vec<String>,
}

impl Job {
    /// Serialized bytes of a command list.
    pub fn encode(cmds: &[String]) -> Vec<u8> {
        let mut out = Vec::new();
        for c in cmds {
            out.extend_from_slice(c.as_bytes());
            out.push(ETX);
        }
        out
    }

    /// Command list split into chunks of at most `max` bytes on command boundaries.
    pub fn chunks(cmds: &[String], max: usize) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        let mut cur = Vec::new();
        for c in cmds {
            if !cur.is_empty() && cur.len() + c.len() + 1 > max {
                chunks.push(std::mem::take(&mut cur));
            }
            cur.extend_from_slice(c.as_bytes());
            cur.push(ETX);
        }
        if !cur.is_empty() {
            chunks.push(cur);
        }
        chunks
    }

    pub fn plot_len(&self) -> usize {
        self.passes.iter().map(|p| p.plot.len()).sum()
    }
}

/// Media set-up: mat type and cuttable area.
pub fn media_commands(mat: MatKind, limits: DeviceLimits) -> Vec<String> {
    let (tg, size) = mat.params();
    let (w_mm, l_mm) = size.map(|(w, l)| (w * 25.4, l * 25.4)).unwrap_or((limits.width_mm, limits.length_mm));
    vec![format!("TG{tg}"), "FN0".into(), "TB50,0".into(), "\\0,0".into(), format!("Z{},{}", su(l_mm), su(w_mm))]
}

/// Tool settings for the Cameo 4 line (incl. Portrait 3), tool holder 1.
pub fn tool_commands(s: &CutSettings) -> Vec<String> {
    let mut c = Vec::new();
    let pressure = s.pressure.clamp(1, 33);
    let speed = s.speed.clamp(1, 30);
    let pen = s.tool == Tool::Pen;
    c.push(format!("FX{pressure},1"));
    c.push("TJ0".into());
    c.push(format!("!{speed},1"));
    c.push(format!("FC0,{},1", su(0.05)));
    c.push(format!("FE{},1", s.lift_corners as u8));
    if pen {
        c.push("FF0,0,1".into());
        c.push("FF0,0,1".into());
    } else {
        // "Sharpen corners" start/end, in 1/10 mm (not device units).
        c.push("FF1,0,1".into());
        c.push("FF1,1,1".into());
    }
    c.push(format!("FX{pressure},1"));
    c.push("TJ3".into());
    let offset = if pen { 0.0 } else { s.blade_offset };
    c.push(format!("FC{},{},1", su(offset), su(0.05)));
    if let (false, Some(d)) = (pen, s.depth) {
        c.push(format!("TF{},1", d.min(10)));
    }
    c
}

/// Complete single-setting set-up (media + tool).
pub fn setup_commands(s: &CutSettings, limits: DeviceLimits) -> Vec<String> {
    let mut c = media_commands(s.mat, limits);
    c.extend(tool_commands(s));
    c
}

/// Move/draw commands for the strokes. Points outside the cuttable area are
/// clamped; returns the number of clamped points.
pub fn plot_commands(strokes: &[Vec<Point>], area_w: f64, area_h: f64) -> (Vec<String>, usize) {
    let mut c = Vec::new();
    let mut clamped = 0;
    let mut clamp = |p: Point| {
        let q = Point::new(p.x.clamp(0.0, area_w), p.y.clamp(0.0, area_h));
        if (q - p).hypot() > 0.05 {
            clamped += 1;
        }
        q
    };
    for s in strokes {
        let Some(first) = s.first() else { continue };
        let p = clamp(*first);
        c.push(format!("M{},{}", su(p.y), su(p.x)));
        let mut last = (su(p.y), su(p.x));
        for q in &s[1..] {
            let q = clamp(*q);
            let cur = (su(q.y), su(q.x));
            if cur != last {
                c.push(format!("D{},{}", cur.0, cur.1));
                last = cur;
            }
        }
    }
    (c, clamped)
}

/// Return the head to the start and release the carriage (media stays loaded).
pub fn trailer_commands() -> Vec<String> {
    trailer_commands_feed(None)
}

/// Like [`trailer_commands`], but with `feed` (mm) the media is pushed out
/// to that position instead – for roll media without a mat, so the cut
/// part can be cut off behind the design.
pub fn trailer_commands_feed(feed: Option<f64>) -> Vec<String> {
    let end = match feed {
        Some(y) if y > 0.0 => format!("M{},0", su(y)),
        _ => "M0,0".into(),
    };
    vec!["L0".into(), "\\0,0".into(), end, "J0".into(), "FN0".into(), "TB50,0".into()]
}

/// Silhouette "type 1" registration marks (print & cut): a filled 5 mm
/// square at the origin (top-left), L-shaped 20 mm corner marks at
/// top-right and bottom-left. All values in mm on the printed page.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegMarks {
    /// Top-left corner of the square mark, from the page's left edge.
    pub origin_x: f64,
    /// … from the page's top edge.
    pub origin_y: f64,
    /// Horizontal distance from the origin to the top-right mark corner.
    pub width: f64,
    /// Vertical distance from the origin to the bottom-left mark corner.
    pub length: f64,
}

pub const REG_SQUARE_MM: f64 = 5.0;
pub const REG_LINE_MM: f64 = 20.0;
pub const REG_LINE_WIDTH_MM: f64 = 0.5;

impl RegMarks {
    pub fn for_page(page_w: f64, page_h: f64, inset: f64) -> Self {
        Self { origin_x: inset, origin_y: inset, width: page_w - 2.0 * inset, length: page_h - 2.0 * inset }
    }

    /// Mark set-up and the automatic search command, as used by
    /// py_silhouette (tested on Portrait models): TB51 = line thickness,
    /// TB53 = line length, width/height measured to the *outside* edges of
    /// the corner brackets.
    pub fn commands(&self) -> (Vec<String>, String) {
        let setup = vec![
            "TB99".to_string(),
            format!("TB51,{}", su(REG_LINE_WIDTH_MM)),
            "TB52,2".into(), // mark type: Cameo/Portrait (square + two brackets)
            format!("TB53,{}", su(REG_LINE_MM)),
            "TB54,0,0".into(), // offset of the square from its expected place
            "TB55,1".into(),   // calibrate scale and rotation from the marks
        ];
        let search = format!("TB123,{},{},117,75", su(self.length), su(self.width));
        (setup, search)
    }
}

/// Builds a job with one pass per `(strokes, settings)` group, cut in the
/// given order. The mat of the first group applies. With registration
/// marks, coordinates become relative to the mark origin.
pub fn build_job(groups: &[(Vec<Vec<Point>>, CutSettings)], limits: DeviceLimits, reg: Option<RegMarks>) -> (Job, usize) {
    let mat = groups.first().map(|g| g.1.mat).unwrap_or(MatKind::Portrait8x12);
    let mut setup = media_commands(mat, limits);
    let mut registration = None;
    if let Some(r) = reg {
        let (reg_setup, search) = r.commands();
        setup.extend(reg_setup);
        registration = Some(search);
    }
    let mut clamped = 0;
    let passes = groups
        .iter()
        .map(|(strokes, s)| {
            let (plot, c) = if let Some(r) = reg {
                let shifted: Vec<Vec<Point>> = strokes
                    .iter()
                    .map(|st| st.iter().map(|p| Point::new(p.x - r.origin_x, p.y - r.origin_y)).collect())
                    .collect();
                plot_commands(&shifted, r.width, r.length)
            } else {
                let (w, h) = match mat.params().1 {
                    Some((w, l)) => ((w * 25.4).min(limits.width_mm), l * 25.4),
                    None => (limits.width_mm, limits.length_mm),
                };
                plot_commands(strokes, w, h)
            };
            clamped += c;
            Pass { tool: tool_commands(s), plot }
        })
        .collect();
    (Job { setup, registration, passes, trailer: trailer_commands() }, clamped)
}

/// Small test pattern: 10 mm square with a triangle inside, top-left at (x, y).
pub fn test_pattern(x: f64, y: f64) -> Vec<Vec<Point>> {
    let p = |dx: f64, dy: f64| Point::new(x + dx, y + dy);
    vec![
        vec![p(3.0, 7.5), p(5.0, 2.5), p(7.0, 7.5), p(3.0, 7.5)],
        vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0), p(0.5, 0.0)],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_for_vinyl() {
        let s = CutSettings::default();
        let c = setup_commands(&s, PORTRAIT3);
        assert_eq!(&c[..5], &["TG3", "FN0", "TB50,0", "\\0,0", "Z6096,4064"]);
        assert!(c.contains(&"FX10,1".to_string()));
        assert!(c.contains(&"!5,1".to_string()));
        assert!(c.contains(&"FC18,1,1".to_string()), "0.9 mm blade offset = 18 SU");
        assert_eq!(c.last().unwrap(), "TF1,1");
    }

    #[test]
    fn pen_has_no_offset_or_depth() {
        let s = CutSettings { tool: Tool::Pen, depth: Some(3), ..Default::default() };
        let c = setup_commands(&s, PORTRAIT3);
        assert!(c.contains(&"FC0,1,1".to_string()));
        assert!(!c.iter().any(|x| x.starts_with("TF")));
    }

    #[test]
    fn plot_swaps_axes_and_clamps() {
        let strokes = vec![vec![Point::new(10.0, 20.0), Point::new(30.0, 20.0), Point::new(300.0, 20.0)]];
        let (c, clamped) = plot_commands(&strokes, 203.2, 304.8);
        assert_eq!(c[0], "M400,200");
        assert_eq!(c[1], "D400,600");
        assert_eq!(c[2], "D400,4064");
        assert_eq!(clamped, 1);
    }

    #[test]
    fn registration_shifts_coordinates() {
        let reg = RegMarks::for_page(210.0, 297.0, 10.0);
        let strokes = vec![vec![Point::new(30.0, 40.0), Point::new(50.0, 40.0)]];
        let (job, clamped) = build_job(&[(strokes, CutSettings::default())], PORTRAIT3, Some(reg));
        assert_eq!(clamped, 0);
        assert_eq!(job.registration.as_deref(), Some("TB123,5540,3800,117,75"));
        assert_eq!(job.passes[0].plot[0], "M600,400", "40-10 mm down, 30-10 mm across");
        assert!(job.setup.contains(&"TB51,10".to_string()) && job.setup.contains(&"TB53,400".to_string()));
        assert!(job.setup.contains(&"TB54,0,0".to_string()));
    }

    #[test]
    fn passes_keep_order_and_settings() {
        let a = vec![vec![Point::new(10.0, 10.0), Point::new(20.0, 10.0)]];
        let kiss = CutSettings { pressure: 10, depth: Some(1), ..Default::default() };
        let through = CutSettings { pressure: 30, depth: Some(6), ..Default::default() };
        let (job, _) = build_job(&[(a.clone(), kiss), (a, through)], PORTRAIT3, None);
        assert_eq!(job.passes.len(), 2);
        assert!(job.passes[0].tool.contains(&"TF1,1".to_string()));
        assert!(job.passes[1].tool.contains(&"TF6,1".to_string()) && job.passes[1].tool.contains(&"FX30,1".to_string()));
        // Media set-up only once.
        assert_eq!(job.setup.iter().filter(|c| c.starts_with("TG")).count(), 1);
        assert!(!job.passes[1].tool.iter().any(|c| c.starts_with("FN") || c.starts_with("TG")));
    }

    #[test]
    fn roll_feed_moves_past_the_design() {
        assert_eq!(trailer_commands_feed(Some(120.0))[2], "M2400,0");
        assert_eq!(trailer_commands_feed(None), trailer_commands());
    }

    #[test]
    fn chunks_respect_command_boundaries() {
        let plot: Vec<String> = (0..500).map(|i| format!("D{i},{i}")).collect();
        let chunks = Job::chunks(&plot, 1024);
        assert!(chunks.len() > 1);
        for ch in &chunks {
            assert!(ch.len() <= 1024);
            assert_eq!(*ch.last().unwrap(), ETX);
        }
        let total: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(total, Job::encode(&plot).len());
    }
}
