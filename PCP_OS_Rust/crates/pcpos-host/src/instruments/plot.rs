//! A deterministic Plotters SVG of bounded exact values. No file or register access.
use super::Outcome;
use num_bigint::BigInt;
use num_integer::Integer;
use pcpos_core::{sha256_hex, value::Value};
use plotters_backend::{
    text_anchor::{HPos, Pos, VPos},
    BackendColor, BackendTextStyle, DrawingBackend, FontFamily,
};
use plotters_svg::SVGBackend;
use std::{cmp::Ordering, fmt, io};

pub const MAX_INPUT: usize = 131_072;
const WIDTH: i32 = 900;
const LEFT: i32 = 155;
const RIGHT: i32 = 775;
const PANEL: i32 = 165;

fn fields(obj: &Value, expected: &[&str]) -> Result<(), String> {
    let Value::Map(entries) = obj else {
        return Err("expected an object".into());
    };
    if entries.len() != expected.len() || expected.iter().any(|k| !entries.contains_key(*k)) {
        return Err(format!(
            "expected exactly these fields: {}",
            expected.join(", ")
        ));
    }
    Ok(())
}

fn label<'a>(obj: &'a Value, key: &str, limit: usize) -> Result<&'a str, String> {
    let text = obj
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be text"))?;
    if text.is_empty()
        || text.trim() != text
        || text.len() > limit
        || text.contains("  ")
        || !text.bytes().all(|b| b == b' ' || b.is_ascii_graphic())
    {
        return Err(format!(
            "{key} must use 1 to {limit} printable ASCII characters, with single spaces inside"
        ));
    }
    Ok(text)
}

struct Number {
    n: BigInt,
    d: BigInt,
}

impl Number {
    fn read(value: &Value) -> Result<Self, String> {
        if let Some(n) = value.as_int() {
            return Ok(Self {
                n: n.into(),
                d: 1.into(),
            });
        }
        let s = value
            .as_str()
            .ok_or("expected an integer or an exact rational string")?;
        let (n, d) = s.split_once('/').unwrap_or((s, "1"));
        let digits =
            |s: &str| !s.is_empty() && s.len() <= 256 && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(n.strip_prefix('-').unwrap_or(n)) || !digits(d) {
            return Err(
                "a rational needs at most 256 decimal digits per part and a positive denominator"
                    .into(),
            );
        }
        let n: BigInt = n.parse().map_err(|_| "invalid numerator")?;
        let d: BigInt = d.parse().map_err(|_| "invalid denominator")?;
        if d == 0.into() {
            return Err("a denominator cannot be zero".into());
        }
        let g = n.gcd(&d);
        Ok(Self {
            n: n / &g,
            d: d / g,
        })
    }
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.n * &other.d).cmp(&(&other.n * &self.d))
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.d == 1.into() {
            write!(f, "{}", self.n)
        } else {
            write!(f, "{}/{}", self.n, self.d)
        }
    }
}

struct Axis {
    lo: Number,
    hi: Number,
}

impl Axis {
    fn read(input: &Value, field: &str) -> Result<Self, String> {
        let Some(Value::List(pair)) = input.get(field) else {
            return Err(format!("{field} must have two endpoints"));
        };
        let [lo, hi] = pair.as_slice() else {
            return Err(format!("{field} must have two endpoints"));
        };
        let axis = Self {
            lo: Number::read(lo)?,
            hi: Number::read(hi)?,
        };
        if axis.lo.cmp(&axis.hi) != Ordering::Less {
            return Err(format!("{field} must increase strictly"));
        }
        if axis.lo.to_string().len() > 16 || axis.hi.to_string().len() > 16 {
            return Err(format!(
                "{field} endpoint labels must fit 16 characters in lowest terms"
            ));
        }
        Ok(axis)
    }
    /// Round span*(v-lo)/(hi-lo) to the nearest pixel, with ties above. No float conversion.
    fn pixel(&self, v: &Number, span: i32) -> Result<i32, String> {
        if v.cmp(&self.lo) == Ordering::Less || v.cmp(&self.hi) == Ordering::Greater {
            return Err("value outside its declared axis; no points were drawn".into());
        }
        let numerator = (&v.n * &self.lo.d - &self.lo.n * &v.d) * &self.hi.d * span;
        let denominator = (&self.hi.n * &self.lo.d - &self.lo.n * &self.hi.d) * &v.d;
        let rounded = (numerator + &denominator / 2) / denominator;
        i32::try_from(rounded).map_err(|_| "projected coordinate is outside pixel range".into())
    }
}

struct Font {
    size: f64,
    align: HPos,
    color: BackendColor,
}

impl BackendTextStyle for Font {
    type FontError = io::Error;
    fn color(&self) -> BackendColor {
        self.color
    }
    fn size(&self) -> f64 {
        self.size
    }
    fn family(&self) -> FontFamily<'_> {
        FontFamily::Monospace
    }
    fn anchor(&self) -> Pos {
        Pos::new(self.align, VPos::Center)
    }
    // The SVG backend writes text elements and needs no native font metrics or raster callbacks.
    fn layout_box(&self, _: &str) -> io::Result<((i32, i32), (i32, i32))> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn draw<E, F: FnMut(i32, i32, BackendColor) -> Result<(), E>>(
        &self,
        _: &str,
        _: (i32, i32),
        _: F,
    ) -> io::Result<Result<(), E>> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

const INK: BackendColor = BackendColor {
    rgb: (30, 42, 58),
    alpha: 1.0,
};
const GRID: BackendColor = BackendColor {
    rgb: (212, 220, 226),
    alpha: 1.0,
};
const BLUE: BackendColor = BackendColor {
    rgb: (4, 99, 121),
    alpha: 1.0,
};
const WHITE: BackendColor = BackendColor {
    rgb: (255, 255, 255),
    alpha: 1.0,
};

pub struct Rendered {
    pub svg: String,
    input_sha: String,
    points: usize,
    narrow: usize,
}

pub fn render(input: &Value) -> Result<Rendered, String> {
    let canonical = input.canonical_json();
    if canonical.len() > MAX_INPUT {
        return Err("canonical plot exceeds 128 KiB".into());
    }
    fields(
        input,
        &[
            "title",
            "provenance",
            "source",
            "x_label",
            "y_label",
            "x_domain",
            "y_domain",
            "series",
        ],
    )?;
    let title = label(input, "title", 54)?;
    let source = label(input, "source", 88)?;
    let x_name = label(input, "x_label", 38)?;
    let y_name = label(input, "y_label", 38)?;
    let provenance = match input.get("provenance").and_then(Value::as_str) {
        Some("model") => "Finite model calculation",
        Some("observed") => "Reported observations",
        Some("mathematics") => "Mathematical data",
        _ => return Err("provenance must be model, observed or mathematics".into()),
    };
    let xs = Axis::read(input, "x_domain")?;
    let ys = Axis::read(input, "y_domain")?;
    let Some(Value::List(series)) = input.get("series") else {
        return Err("series must be an array".into());
    };
    if !(1..=4).contains(&series.len()) {
        return Err("supply 1 to 4 series".into());
    }
    let height = 120 + PANEL * series.len() as i32 + 90;
    let mut svg = String::new();
    let mut points = 0;
    let mut narrow = 0;
    let input_sha = sha256_hex(&canonical);
    {
        let mut g = SVGBackend::with_string(&mut svg, (WIDTH as u32, height as u32));
        let err = |e| format!("SVG rendering failed: {e}");
        let text = |g: &mut SVGBackend<'_>, txt: &str, p, size, align| {
            g.draw_text(
                txt,
                &Font {
                    size,
                    align,
                    color: INK,
                },
                p,
            )
            .map_err(err)
        };
        g.draw_rect((0, 0), (WIDTH, height), &WHITE, true)
            .map_err(err)?;
        text(&mut g, title, (24, 29), 21., HPos::Left)?;
        text(
            &mut g,
            &format!("{provenance} | {source}"),
            (24, 58),
            13.,
            HPos::Left,
        )?;
        text(&mut g, y_name, (24, 92), 15., HPos::Left)?;
        for (index, s) in series.iter().enumerate() {
            fields(s, &["name", "data"])?;
            let name = label(s, "name", 48)?;
            if series[..index]
                .iter()
                .any(|q| q.get("name").and_then(Value::as_str) == Some(name))
            {
                return Err("series names must be distinct".into());
            }
            let Some(Value::List(rows)) = s.get("data") else {
                return Err("data must be an array".into());
            };
            if !(1..=64).contains(&rows.len()) {
                return Err("each series takes 1 to 64 rows".into());
            }
            let top = 126 + PANEL * index as i32;
            let bottom = top + 108;
            text(&mut g, name, (LEFT, top - 15), 15., HPos::Left)?;
            for (height, v) in [(top, &ys.hi), (bottom, &ys.lo)] {
                g.draw_line((LEFT, height), (RIGHT, height), &GRID)
                    .map_err(err)?;
                text(&mut g, &v.to_string(), (LEFT - 9, height), 13., HPos::Right)?;
            }
            let mut previous_x = None;
            for (r, row) in rows.iter().enumerate() {
                let Value::List(a) = row else {
                    return Err("row must be [x,lower,upper]".into());
                };
                let [x, lo, hi] = a.as_slice() else {
                    return Err("row must be [x,lower,upper]".into());
                };
                let x = LEFT + xs.pixel(&Number::read(x)?, RIGHT - LEFT)?;
                if previous_x.is_some_and(|p| p >= x) {
                    return Err(format!(
                        "{name}, row {}: x must strictly increase and have a distinct pixel",
                        r + 1
                    ));
                }
                previous_x = Some(x);
                let lo = Number::read(lo)?;
                let hi = Number::read(hi)?;
                let order = lo.cmp(&hi);
                if order == Ordering::Greater {
                    return Err(format!("{name}, row {}: the interval is reversed", r + 1));
                }
                let y0 = bottom - ys.pixel(&lo, bottom - top)?;
                let y1 = bottom - ys.pixel(&hi, bottom - top)?;
                match (order, y0 == y1) {
                    (Ordering::Equal, _) => g.draw_circle((x, y0), 3, &BLUE, true).map_err(err)?,
                    (_, true) => {
                        g.draw_circle((x, y0), 4, &BLUE, false).map_err(err)?;
                        narrow += 1;
                    }
                    _ => {
                        g.draw_line((x, y0), (x, y1), &BLUE).map_err(err)?;
                        for y in [y0, y1] {
                            g.draw_line((x - 3, y), (x + 3, y), &BLUE).map_err(err)?;
                        }
                    }
                }
                points += 1;
            }
            text(
                &mut g,
                &xs.lo.to_string(),
                (LEFT, bottom + 17),
                13.,
                HPos::Center,
            )?;
            text(
                &mut g,
                &xs.hi.to_string(),
                (RIGHT, bottom + 17),
                13.,
                HPos::Center,
            )?;
        }
        text(
            &mut g,
            x_name,
            ((LEFT + RIGHT) / 2, height - 75),
            15.,
            HPos::Center,
        )?;
        text(&mut g,"Filled dot: point. Bar: interval. Open circle: nonzero interval with coincident rounded endpoints.",(24,height-45),13.,HPos::Left)?;
        text(
            &mut g,
            &format!("Input SHA-256  {input_sha}"),
            (24, height - 20),
            12.,
            HPos::Left,
        )?;
        g.present().map_err(err)?;
    }
    Ok(Rendered {
        svg,
        input_sha,
        points,
        narrow,
    })
}

pub fn check(args: &Value) -> Result<Outcome, String> {
    fields(args, &["plot", "expected_svg_sha256"])?;
    let expected = args
        .get("expected_svg_sha256")
        .and_then(Value::as_str)
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("expected_svg_sha256 must be a 64-digit hex digest")?;
    let rendered = render(args.get("plot").ok_or("plot is required")?)?;
    let hash = sha256_hex(rendered.svg.as_bytes());
    let holds = hash.eq_ignore_ascii_case(expected);
    let verdict = if holds { "matches" } else { "differs" };
    let text = format!(
        "SVG {} bytes, SHA-256 {hash}. Expected {expected}. Rendered {} rows including {} intervals +         with coincident rounded endpoints. Canonical plot SHA-256 {}. SVG digest {verdict}.",
        rendered.svg.len(), rendered.points, rendered.narrow, rendered.input_sha
    );
    Ok(Outcome { holds, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../../docs/plots/finite_walk.json");

    fn input(data: &str) -> Value {
        let json = format!(
            r#"{{"title":"R & D <lab>","source":"test","provenance":"model",
          "x_label":"x","y_label":"y","x_domain":[0,1],"y_domain":[-1,1],
          "series":[{{"name":"a","data":{data}}}]}}"#
        );
        Value::parse(json.as_bytes()).unwrap()
    }

    #[test]
    fn projection_keeps_very_close_fractions_and_extreme_integers() {
        let huge = format!("1{}", "0".repeat(250));
        let top = format!("{}1", "0".repeat(250));
        let observed = render(&input(&format!(r#"[[0,0,"{top}/{huge}"],[1,-1,1]]"#))).unwrap();
        assert_eq!(observed.narrow, 1);
        assert_eq!(observed.points, 2);
        assert!(observed.svg.contains("&lt;lab&gt;") && observed.svg.contains("&amp;"));
        assert!(!observed.svg.contains("<lab>"));
        let axis = Axis {
            lo: Number::read(&Value::Int(i64::MIN)).unwrap(),
            hi: Number::read(&Value::Int(i64::MAX)).unwrap(),
        };
        assert_eq!(
            axis.pixel(&Number::read(&Value::Int(0)).unwrap(), 620)
                .unwrap(),
            310
        );
    }

    #[test]
    fn malformed_or_unprojectable_rows_refuse_the_entire_plot() {
        for invalid in [
            r#"[[0,0,"1/0"]]"#,
            r#"[[0,0,"1e-8"]]"#,
            r#"[[0,0,true]]"#,
            r#"[[0,0,1],[1,2,2]]"#,
            r#"[[0,1,0]]"#,
            r#"[[0,0,0],[0,0,0]]"#,
            r#"[[0,0,0],["1/2000",0,1]]"#,
            r#"[[1,0,0],[0,0,1]]"#,
        ] {
            assert!(render(&input(invalid)).is_err(), "{invalid}");
        }
        assert!(render(&input(&format!(r#"[[0,0,"1/{}"]]"#, "1".repeat(257)))).is_err());
    }

    #[test]
    fn hash_of_the_rendered_artifact_distinguishes_inputs_even_at_one_pixel() {
        let plot = input(r#"[[0,0,"1/100000"]]"#);
        let other = input(r#"[[0,0,"1/100001"]]"#);
        let a = render(&plot).unwrap();
        assert_ne!(a.svg, render(&other).unwrap().svg);
        let pin = sha256_hex(a.svg.as_bytes());
        let args = |p: Value, hash: &str| {
            Value::Map(
                [
                    ("plot".into(), p),
                    ("expected_svg_sha256".into(), Value::Str(hash.into())),
                ]
                .into(),
            )
        };
        assert!(check(&args(plot.clone(), &pin)).unwrap().holds);
        assert!(!check(&args(other, &pin)).unwrap().holds);
        assert!(check(&args(plot, "bad")).is_err());
        assert_eq!(
            render(&Value::parse(FIXTURE.as_bytes()).unwrap())
                .unwrap()
                .points,
            5
        );
    }

    #[test]
    fn shipped_figures_reproduce_exact_svg_bytes() {
        for (input, expected) in [
            (
                FIXTURE,
                include_str!("../../../../docs/plots/finite_walk.svg"),
            ),
            (
                include_str!("../../../../docs/plots/cesium_counts.json"),
                include_str!("../../../../docs/plots/cesium_counts.svg"),
            ),
        ] {
            assert_eq!(
                render(&Value::parse(input.as_bytes()).unwrap())
                    .unwrap()
                    .svg,
                expected
            );
        }
    }
}
